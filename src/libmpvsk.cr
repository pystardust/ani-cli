#
# LIBMPVSK
#   Crystal library for talking to mpv JSON IPC socket
#

require "socket"
require "json"

struct Mpvsk
  @@sock = UNIXSocket.new(ENV["HOME"] + "/.mpvsock")

  def self.put(command : Array)
    @@sock.puts %({ "command": #{command} })
  end

  def self.put_get(command : Array)
    @@sock.puts %({ "command": #{command} })
    response = JSON.parse(@@sock.gets.not_nil!).as_h
    return response
  end

  def self.get_property(property : String, as_string : Bool)
    if as_string
      put_get(["get_property_string", property])
    else
      put_get(["get_property", property])
    end
  end

  def self.set_property(property : String, value)
    put(["set_property", property, value])
  end

  def self.get_pos
    get_property("time-pos", true)["data"]
  end
end
