local mp = require("mp")
local io = require("io")
local os = require("os")
local math = require("math")

local ani_cli_state_dir = os.getenv("HOME").."/.local/state/ani-cli/"
local anime_eps_history_file_loc = ani_cli_state_dir.."ani-eps-hsts/"
local anime_tmp_history_file_loc = ani_cli_state_dir.."tmp/"

AnimeEpTimestamp = {
    media_title = "",
    anime_name = "",
    anime_ep = 0,
    ep_timestamp = 0,
    ep_duration_with_ed = 0,
    ep_duration_without_ed = 0,
    anime_ep_timestamp_file_path = "",
}

function AnimeEpTimestamp:new()
    local obj = {}
    setmetatable(obj, self)
    self.__index = self

    return obj
end

function AnimeEpTimestamp:GetProps()
    -- get props from video
    self.media_title = mp.get_property("media-title").." Episode 3"
    self.anime_name = string.gsub(self.media_title, " [Ee]pisode [0-9]+", "")
    self.anime_ep = string.gsub(self.media_title, ".*[Ee]pisode ", "")
    -- self.ep_timestamp = math.floor(mp.get_property("time-pos"))
    self.ep_duration_with_ed = math.floor(mp.get_property("duration"))
    self.ep_duration_without_ed = self.ep_duration_with_ed - 60

    -- string representing file locations
    self.anime_ep_timestamp_file_path = ani_cli_state_dir.."ani-eps-hsts/"..self.anime_name
    self.anime_ep_hist_file_path = ani_cli_state_dir.."ani-hsts"

    -- function to read from timestamp file
    AnimeEpTimestamp:ReadFromTimeStampFile()
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
  local content_table = { }

  if anime_ep_timestamp_file_obj:read("l") == nil then
    content_table = { self.anime_ep.." - "..self.ep_timestamp }
  end

  -- if anime_ep_timestamp_file_obj:read("l") ~= nil then
  --   for i in io.lines(self.anime_ep_timestamp_file_path) do
  --    print(i)
  --    content_table.insert(i)
  --   end
  -- end
  --
  -- anime_ep_timestamp_file_obj:close()

  return content_table
end



function AnimeEpTimestamp:WriteTimeStampOnQuit()
  self.anime_ep_timestamp_file_obj = assert(io.open(self.anime_ep_timestamp_file_path, "a+"))
  self.anime_ep_timestamp_file_obj:write(self.anime_ep.." - "..self.ep_timestamp.."\n")
  -- self.anime_ep in timestamp_file.readlines()
  -- string.replace("self.anime_ep - self.anime_ep_timestamp", "self.anime_ep - self.curr_pos")
  self.anime_ep_timestamp_file_obj:close()
end


function AnimeEpTimestamp:EditEpHistoryOnQuit()
  -- local watch_hist_f_obj = assert(io.open(self.anime_ep_hist_file_path, "r+"))

  -- -- ani_hsts replace & match regex
  -- local ep_num_regex = "^%d+%.?%d*"
  -- local anime_hash_regex = "%w+%s"
  -- local total_eps_in_bracket = "%([0-9]+ episodes%)"
  -- -- end of regex def.s

  -- watch_hist_f_obj:close()
end


AnimeEpTimestampObj = AnimeEpTimestamp:new()
function Onload()
  AnimeEpTimestampObj:GetProps()
  -- AnimeEpTimestampObj:ShowDeets()
  AnimeEpTimestampObj:ReadFromTimeStampFile()
  AnimeEpTimestampObj:UpdateTimeStampVars()
  -- AnimeEpTimestampObj:EditEpHistoryOnQuit()

  -- write to file on shutdown
  mp.register_event("shutdown", AnimeEpTimestampObj:WriteTimeStampOnQuit())
end

-- mp.set_property("save-position-on-quit", "yes")
-- mp.set_property("watch-later-directory", "/home/blank/.local/state/ani-cli/watch_later_tmp/")
-- mp.set_property("", "")

mp.register_event("file-loaded", Onload)


