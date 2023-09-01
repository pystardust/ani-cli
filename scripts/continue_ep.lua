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
    ep_duration_with_op_ed = 0,
    ep_duration_without_op_ed = 0,
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
    self.ep_timestamp = math.floor(mp.get_property("time-pos"))
    self.ep_duration_with_op_ed = math.floor(mp.get_property("duration"))
    self.ep_duration_without_op_ed = self.ep_duration_with_op_ed - 90

    -- string representing file locations
    self.anime_ep_timestamp_file_path = ani_cli_state_dir.."ani-eps-hsts/"..self.anime_name
    self.anime_ep_hist_file_path = ani_cli_state_dir.."ani-hsts"
end

function AnimeEpTimestamp:ShowDeets()
  print("self.media_title ->",self.media_title)
  print("self.anime_name ->",self.anime_name)
  print("self.anime_ep ->",self.anime_ep)
  print("self.anime_ep_timestamp_file ->",self.anime_ep_timestamp_file)
  print("self.ep_timestamp ->",self.ep_timestamp)
  print("self.ep_duration_with_op_ed ->",self.ep_duration_with_op_ed)
  print("self.ep_duration_without_op_ed ->",self.ep_duration_without_op_ed)
end



function AnimeEpTimestamp:UpdateTimeStampVars()

  if (mp.get_property("filename") == nil) or (self.ep_timestamp == nil ) then
    return
  end

  while ((self.ep_timestamp <= self.ep_duration_with_op_ed) and
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
  local anime_ep_timestamp_file_obj = io.open(self.anime_ep_timestamp_file_path, "a+")
  -- ep_timestamp = io.open("timestamp_file", "r")[-1]
  -- ep = ep_timestamp[1]
  -- if ep == curr_ep
  --  start_pos = ep_timestamp[1]
  --  self.start_pos = start_pos
  if anime_ep_timestamp_file_obj ~= nil then anime_ep_timestamp_file_obj:close() end
end


function ()
end


function AnimeEpTimestamp:edit_file_at_content_pos(f_path, line_pos, write_content)

  local file_obj = assert(io.open(f_path, "a+"))
  local curr_line_pos = 0 -- Start at the beginning of the file

  for _ = 1, line_pos - 1 do
      f_path:read("*line")
      curr_line_pos = f_path:seek()
  end

  f_path:seek("set", curr_line_pos)
  f_path:write(write_content)
  file_obj:close()

end


function AnimeEpTimestamp:WriteTimeStampOnQuit()
  -- self.anime_ep_timestamp_file_obj = assert(io.open(self.anime_ep_timestamp_file_path, "a+"))
  -- self.anime_ep_timestamp_file_obj:write(self.anime_ep.." - "..self.ep_timestamp)
  -- -- self.anime_ep in timestamp_file.readlines()
  -- -- string.replace("self.anime_ep - self.anime_ep_timestamp", "self.anime_ep - self.curr_pos")
  -- self.anime_ep_timestamp_file_obj:close()
end


function AnimeEpTimestamp:EditEpHistoryOnQuit()
  print("string =>", self.anime_ep_hist_file_path)
  local watch_hist_f_obj = assert(io.open(self.anime_ep_hist_file_path, "r+"))

  -- ani_hsts replace & match regex
  local ep_num_regex = "^%d+%.?%d*"
  local anime_hash_regex = "%w+%s"
  local total_eps_in_bracket = "%([0-9]+ episodes%)"
  -- end of regex def.s

  for i in watch_hist_f_obj:lines()
  do
    local anime_name = i:gsub(ep_num_regex.."%s", ""):gsub("^"..anime_hash_regex, ""):gsub(total_eps_in_bracket, "")
    local anime_ep = i:match(ep_num_regex)
    print(anime_name)
  end
  watch_hist_f_obj:close()
end


-- AnimeEpTimestampObj = {}
AnimeEpTimestampObj = AnimeEpTimestamp:new()
function Onload()
  AnimeEpTimestampObj:GetProps()
  AnimeEpTimestampObj:ShowDeets()
  -- AnimeEpTimestampObj:ReadFromTimeStampFile()
  -- AnimeEpTimestampObj:UpdateTimeStampVars()
  -- AnimeEpTimestampObj:EditEpHistoryOnQuit()

  -- write to file on shutdown
  mp.register_event("shutdown", AnimeEpTimestampObj:WriteTimeStampOnQuit())
end

mp.set_property("save-position-on-quit", "yes")
mp.set_property("watch-later-directory", "/home/blank/.local/state/ani-cli/watch_later_tmp/")
-- mp.set_property("", "")

mp.register_event("file-loaded", Onload)


