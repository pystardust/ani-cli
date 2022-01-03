#
#  ANI-CLI -- Crystal rewrite
#  https://github.com/pystardust/ani-cli
#

#
# Dependencies
require "clim"
require "term-prompt"
require "term-screen"
require "file_utils"
require "socket"
require "./libgogo"

#
# Constants
PLAYER_FN = "mpv"
PROGRAM   = "ani-cli"
PROMPT    = Term::Prompt.new(prefix: "\033[1;36m[ani-cli]\033[0m ")
HOME = ENV["HOME"]
HELP_STRING = "
help    Show this message
quit    Quit ani-cli
search  Find series with the given term
"

#
# Aliases
alias C = Color

def newline
  puts nil
end

#
# Colors
class Color
  RED     = "\033[1;31m"
  GREEN   = "\033[1;32m"
  YELLOW  = "\033[1;33m"
  BLUE    = "\033[1;34m"
  MAGENTA = "\033[1;35m"
  CYAN    = "\033[1;36m"
  RESET   = "\033[0m"
end

#
# Welcome message
puts C::MAGENTA + " Welcome to ani-cli! ".center(Term::Screen.width, '=')
puts C::YELLOW + "\nType `help` and press enter to get started\n\n"

Signal::INT.trap do
  puts "Type `quit` and press enter to quit"
end

#
# Command line tool
module AniCli
  class Cli < Clim
    main do
      # Boring stuff
      desc "ani-cli: The simple, beautiful anime watching command line utility"
      usage "ani-cli --help"
      version "Version 0.1.0"

      #
      # Interactive prompt
      run do |opts, args|
        unless File.exists?("#{HOME}/.config/ani-cli/watched.yaml") # If watched.yaml doesn't exist...
          FileUtils.mkdir("#{HOME}/.config/ani-cli")                # Create a folder for it...
          FileUtils.touch("#{HOME}/.config/ani-cli/watched.yaml")   # and create the file itself
        end

        loop do
          command = PROMPT.ask("=> ")
          case command
          when "help"
            puts HELP_STRING
          when "search"
            Flows.search
          when "quit"
            exit
          end
        end
      end
    end
  end

  class Flows
    def self.search
      query = PROMPT.ask("Search anime: ", required: true) # Prompt user for search term
      series_options = Libgogo.search_anime(query.not_nil!)            # Get results

      # Display results to user
      series_option = PROMPT.select("Choose a series", required: true) do |list|
        series_options.each do |key, value|
          list.choice(key, value)
        end
      end

      episode_options = Libgogo.ep_count(series_option.not_nil!) # Get episode count (range)

      # Display list of episodes to user
      episode_option = PROMPT.select("Choose an episode", required: true) do |list|
        episode_options.each do |episode|
          list.choice("Ep. #{episode}", episode.to_s)
        end
      end

      episode_id = Libgogo.get_ep_id(series_option.not_nil!, episode_option.not_nil!.to_i) # Get the episode id
      episode_url = Libgogo.get_video_url(episode_id)                                      # Use the episode id to get a stream
      FileUtils.touch("#{HOME}/.ani-cli_mpv")                                       # Prepare a socket for mpv

      # Start mpv with some custom options
      `nohup mpv --idle=yes --force-window=yes --input-ipc-server=#{HOME}/.ani-cli_mpv --ytdl-raw-options=external-downloader=aria2,external-downloader-args:aria2c="-x 10 -s 10 -k 100K" > /dev/null 2>&1 &`
      sleep 3

      # Connect to mpv's socket
      sock = UNIXSocket.new("#{HOME}/.ani-cli_mpv")

      # Load the video into mpv
      print C::BLUE + "Loading video..."
      sock.puts %({ "command": #{["loadfile", episode_url]} })
      puts C::GREEN + " done!\n"
    end
  end
end

# Start Clim
AniCli::Cli.start(ARGV)
