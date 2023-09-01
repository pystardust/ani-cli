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
  self.media_title = mp.get_property("media-title"):gsub(".mp4", "")
  -- self.media_title = mp.get_property("media-title")
  self.anime_name = string.gsub(self.media_title, " [Ee]pisode [0-9]+", "")
  self.anime_ep = string.gsub(self.media_title, ".*[Ee]pisode ", "")
  -- self.ep_timestamp = math.floor(mp.get_property("time-pos"))
  self.ep_duration_with_ed = math.floor(mp.get_property("duration"))
  self.ep_duration_without_ed = self.ep_duration_with_ed - 60

  -- string representing file locations
  self.anime_ep_timestamp_file_path = ani_cli_state_dir.."ani-eps-hsts/"..self.anime_name
  self.anime_ep_hist_file_path = ani_cli_state_dir.."ani-hsts"

  self.content_table = AnimeEpTimestampObj:ReadFromTimeStampFile()


  if next(self.content_table) == nil then
    print("shit")
  end

  if next(self.content_table) ~= nil then
    for index in ipairs(self.content_table) do
      if self.content_table[index]:match(self.anime_ep.." - ") then
        self.ep_start_pos_timestamp = self.content_table[index]:gsub("^"..self.anime_ep.." %- ", "")
        break
      end
    end
  end

end




function AnimeEpTimestamp:ShowDeets()
  print("self.media_title ->",self.media_title)
  print("self.anime_name ->",self.anime_name)
  print("self.anime_ep ->",self.anime_ep)
  print("self.anime_ep_timestamp_file ->",self.anime_ep_timestamp_file)
  print("self.ep_timestamp ->",self.ep_timestamp)
  print("self.ep_duration_with_ed ->",self.ep_duration_with_ed)
  print("self.ep_duration_without_ed ->",self.ep_duration_without_ed)
end



function AnimeEpTimestamp:UpdateTimeStampVars()

  -- function to read from timestamp file
  if (mp.get_property("filename") == nil) or (self.ep_timestamp == nil ) then
    return
  end

  while ((self.ep_timestamp <= self.ep_duration_without_ed) and
         (mp.get_property_bool("pause") == false) and
         (mp.get_property("time-pos") ~= nil))
  do
    print(self.ep_timestamp)
    self.ep_timestamp = math.floor(mp.get_property("time-pos"))
    os.execute("sleep 1")
  end

  while (mp.get_property_bool("pause", nil) == true) do
    os.execute("sleep 1")
  end
  self:UpdateTimeStampVars()

end



function AnimeEpTimestamp:ReadFromTimeStampFile()
  local anime_ep_timestamp_file_obj = assert(io.open(self.anime_ep_timestamp_file_path, "a+"))

  if anime_ep_timestamp_file_obj:read("l") == nil then
    self.content_table = { self.anime_ep.." - "..self.ep_timestamp }
  end

  if anime_ep_timestamp_file_obj:read("l") ~= nil then
    for i in io.lines(self.anime_ep_timestamp_file_path) do
     table.insert(self.content_table, i)
    end
  end

  anime_ep_timestamp_file_obj:close()

  return self.content_table
end



function AnimeEpTimestamp:WriteTimeStampOnQuit()
  if next(self.content_table) == nil then
    self.content_table = { self.anime_ep.." - "..self.ep_timestamp }
  end

  if next(self.content_table) ~= nil then
    for index in ipairs(self.content_table) do
      if self.content_table[index]:match(self.anime_ep.." - ") then

        if self.ep_timestamp >= self.ep_duration_without_ed then
          table.remove(self.content_table, index)

        elseif self.ep_timestamp < self.ep_duration_without_ed then
          self.content_table[index] = self.anime_ep.." - "..self.ep_timestamp
        end

        break
      end
    end
  end

  self.anime_ep_timestamp_file_obj = assert(io.open(self.anime_ep_timestamp_file_path, "w+"))
  for index in ipairs(self.content_table) do
    self.anime_ep_timestamp_file_obj:write(self.content_table[index].."\n")
  end
  self.anime_ep_timestamp_file_obj:close()

  -- AnimeEpTimestampObj:EditEpHistoryOnQuit()
end


function AnimeEpTimestamp:EditEpHistoryOnQuit()
  local ani_hsts_table = {}
  print("mofo ==> ",self.anime_ep_hist_file_path)
  for i in io.lines(self.anime_ep_hist_file_path) do
    ani_hsts_table.insert(i)
  end

  print("fuck")
  for index in ipairs(ani_hsts_table) do
    if ani_hsts_table[index]:match(self.anime_name) then
      print(ani_hsts_table[index].." => subbing")
      if self.ep_timestamp >= self.ep_duration_without_ed then
        local curr_ep ani_hsts_table[index]:gsub("^%d+", self.anime_ep)
        print(curr_ep)
      end
    end
  end
end


AnimeEpTimestampObj = AnimeEpTimestamp:new()
function Onload()
  AnimeEpTimestampObj:GetProps()
  AnimeEpTimestampObj:ShowDeets()
  mp.set_property("time-pos", AnimeEpTimestampObj.ep_start_pos_timestamp)
  AnimeEpTimestampObj:UpdateTimeStampVars()

  -- write to file on shutdown
  mp.register_event("shutdown", AnimeEpTimestampObj:WriteTimeStampOnQuit())
end

-- mp.set_property("save-position-on-quit", "yes")
-- mp.set_property("watch-later-directory", "/home/blank/.local/state/ani-cli/watch_later_tmp/")
-- mp.set_property("", "")

mp.register_event("file-loaded", Onload)


