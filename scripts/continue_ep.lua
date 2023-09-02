local mp = require("mp")
local io = require("io")
local os = require("os")
local math = require("math")

local ani_cli_state_dir = os.getenv("HOME").."/.local/state/ani-cli/"
local anime_eps_history_file_loc = ani_cli_state_dir.."ani-eps-hsts/"



AnimeEpTimestamp = {
    media_title = "",
    anime_name = "",
    anime_ep = 0,
    ep_timestamp = 0,
    ep_duration_with_ed = 0,
    ep_duration_without_ed = 0,
    ep_start_pos_timestamp = 0,
    anime_ep_timestamp_file_path = "",
    content_table = { }
}

function AnimeEpTimestamp:new()
    local obj = {}
    setmetatable(obj, self)
    self.__index = self

    return obj
end

function AnimeEpTimestamp:GetProps()

  -- get props from video
  self.media_title = mp.get_property("media-title")
  local file_extensions = { "mp4", "mov", "m4a", "3gp", "3g2", "mj2" }

  for index in ipairs(file_extensions) do
    local extension_pattern = "%."..file_extensions[index].."$"
    if self.media_title:match(extension_pattern) then
      self.media_title = self.media_title:gsub(extension_pattern, "")
      break
    end
  end

  self.anime_name = string.gsub(self.media_title, " [Ee]pisode [0-9]+", "")
  self.anime_ep = string.gsub(self.media_title, ".*[Ee]pisode ", "")
  self.ep_duration_with_ed = math.floor(mp.get_property("duration"))
  self.ep_duration_without_ed = self.ep_duration_with_ed - 100

  -- string representing file locations
  -- self.anime_ep_timestamp_file_path = ani_cli_state_dir.."ani-eps-hsts/"..self.anime_name
  self.anime_ep_timestamp_file_path = anime_eps_history_file_loc..self.anime_name
  self.anime_ep_hist_file_path = ani_cli_state_dir.."ani-hsts"

  self.content_table = AnimeEpTimestampObj:ReadFromTimeStampFile()


  if #self.content_table > 0 then
    for index in ipairs(self.content_table) do
      if self.content_table[index]:match(self.anime_ep.." - ") then
        self.ep_start_pos_timestamp = self.content_table[index]:gsub("^"..self.anime_ep.." %- ", "")
        break
      end
    end
  end

end


function AnimeEpTimestamp:UpdateTimeStampVars()

  while mp.get_property("filename") ~= nil do
    while mp.get_property("time-pos") ~= nil do
      self.ep_timestamp = math.floor(mp.get_property("time-pos"))
      os.execute("sleep 1")
    end

    while mp.get_property("pause") == true do
      os.execute("sleep 1")
    end
  end
end



function AnimeEpTimestamp:ReadFromTimeStampFile()
  local anime_ep_timestamp_file_obj = assert(io.open(self.anime_ep_timestamp_file_path, "a+"))

  for i in io.lines(self.anime_ep_timestamp_file_path) do
   table.insert(self.content_table, i)
  end

  anime_ep_timestamp_file_obj:close()

  return self.content_table
end


function AnimeEpTimestamp:EditEpHistoryOnQuit()
  if self.ep_timestamp <= self.ep_duration_without_ed then
    local ani_hsts_table = { }
    for i in io.lines(self.anime_ep_hist_file_path) do
      table.insert(ani_hsts_table, i)
    end

    for index in ipairs(ani_hsts_table) do
      if ani_hsts_table[index]:match(self.anime_name) then
        ani_hsts_table[index] = ani_hsts_table[index]:gsub("^%d+", self.anime_ep)
      end
    end

    -- local anime_ep_hist_file_obj = assert(io.open(self.anime_ep_hist_file_path, "w+"))
    -- for index in ipairs(ani_hsts_table) do
    --   anime_ep_hist_file_obj:write(ani_hsts_table[index])
    -- end

  end
end



function AnimeEpTimestamp:WriteTimeStampOnQuit()
  if (self.ep_timestamp >= self.ep_duration_without_ed) and (#self.content_table == 0) then
    os.remove(self.anime_ep_timestamp_file_path)
  end

  if self.ep_timestamp <= self.ep_duration_without_ed then
    if #self.content_table == 0 then
      self.content_table = { self.anime_ep.." - "..self.ep_timestamp }
    end

    local ep_found = 0
    for index in ipairs(self.content_table) do
      print(self.content_table[index])
      if self.content_table[index]:match(self.anime_ep.." - ") then

        ep_found = 1
        if self.ep_timestamp >= self.ep_duration_without_ed then
          table.remove(self.content_table, index)

        elseif self.ep_timestamp < self.ep_duration_without_ed then
          self.content_table[index] = self.anime_ep.." - "..self.ep_timestamp
        end

        break
      end
    end

    if ep_found == 0 then
      table.insert(self.content_table, self.anime_ep.." - "..self.ep_timestamp)
    end

    self.anime_ep_timestamp_file_obj = assert(io.open(self.anime_ep_timestamp_file_path, "w+"))
    for index in ipairs(self.content_table) do
      self.anime_ep_timestamp_file_obj:write(self.content_table[index].."\n")
    end
    self.anime_ep_timestamp_file_obj:close()

  end

  -- AnimeEpTimestampObj:EditEpHistoryOnQuit()
end



-- mp.set_property("save-position-on-quit", "yes")
-- mp.set_property("watch-later-directory", "/home/blank/.local/state/ani-cli/watch_later_tmp/")


mp.register_event("file-loaded", function ()
  AnimeEpTimestampObj = AnimeEpTimestamp:new()
  AnimeEpTimestampObj:GetProps()
  mp.set_property("time-pos", AnimeEpTimestampObj.ep_start_pos_timestamp)
  AnimeEpTimestampObj:UpdateTimeStampVars()

  -- write to file on shutdown
  mp.register_event("shutdown", AnimeEpTimestampObj:WriteTimeStampOnQuit())
end)


