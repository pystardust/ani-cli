#
# LIBGOGO
#   Crystal library for piracy streaming site Gogoanime
#

require "http/client"
alias HTTPCli = HTTP::Client

# Scrape new episodes from the homepage
def new_episodes
  body = HTTPCli.get(BASE_URL.gsub("http://", "https://")).body.to_s                               # Fetch the homepage
  data = [] of NamedTuple(title: String, id: String, episode: Int32)                               # Initialize array for results
  body.each_line do |line|                                                                         # Iterate through each line and try to parse
    if captures = line.match(/<a href="\/(?<id>.*)-episode-(?<episode>\d*)" title="(?<title>.*)"/) # If line can be parsed, parse it and store the captures
      data << {title: captures["title"], id: captures["id"], episode: captures["episode"].to_i}    # Append result to array
    end
  end
  return data
end

# Search for anime under the given keyword
def search_anime(query : String)
  query = query.gsub(" ", "-")                                                       # Whitespace to dashes so we can pass it in the URL
  body = HTTP::Client.get("#{BASE_URL}/search.html?keyword=#{query}").body.to_s      # Fetch the results page
  data = [] of NamedTuple(title: String, id: String)                                 # Initialize array for results
  parse_start = Time.utc                                                             # Store parsing start time for later
  body.each_line do |line|                                                           # Iterate through each line and try to parse
    if captures = line.match(/<a href="\/category\/(?<id>.*)" title="(?<title>.*)"/) # If line can be parsed, parse it and store the captures
      data << {title: captures["title"], id: captures["id"]}                         # Append result to array
    end
  end
  # puts "#{(Time.utc - parse_start).milliseconds} ms"  # Calculate parsing time; highly dependent on the power of the hardware, I got 4ms on average
  return data # Return results
end

def get_ep_id(anime : String, episode : Int32)
  body = HTTP::Client.get("#{BASE_URL}/#{anime}-episode-#{episode}").body.to_s          # Fetch the episode's page
  ep_id = body.match(/<a href="#" rel="100".*data-video=".*(?:id([^&]+)&)/).not_nil![1] # Find episode ID
  return ep_id                                                                          # Return episode ID
end

# search_eps () {
# 	# get available episodes for anime_id
# 	anime_id=$1
#
# 	curl -s "$base_url/category/$anime_id" |
# 	sed -n -E '
# 		/^[[:space:]]*<a href="#" class="active" ep_start/{
# 		s/.* '\''([0-9]*)'\'' ep_end = '\''([0-9]*)'\''.*/\2/p
# 		q
# 		}
# 		'
# }

# get_embedded_video_link() {
# 	# get the download page url
# 	anime_id=$1
# 	ep_no=$2
#
# 	# credits to fork: https://github.com/Dink4n/ani-cli for the fix
# 	# dub prefix takes the value "-dub" when dub is needed else is empty
# 	curl -s "$base_url/$anime_id${dub_prefix}-episode-$ep_no" |
# 	sed -n -E '
# 		/^[[:space:]]*<a href="#" rel="100"/{
# 		s/.*data-video="([^"]*)".*/https:\1/p
# 		q
# 		}'
# }

# get_video_quality() {
#
# 	get_links
# 	video_quality=$(curl -s get_links "$video_url" | grep -oE "(http|https):\/\/.*com\/cdn.*expiry=[0-9]*"| sort -V | sed 's/amp;//')
# 	case $quality in
# 		best)
# 			play_link=$(echo "$video_quality" | sort -V | tail -n 1);;
# 		worst)
# 			play_link=$(echo "$video_quality" | sort -V | head -n 1);;
#         	*)
#   	             	play_link=$(echo "$video_quality" | grep -oE "(http|https):\/\/.*com\/cdn.*"${quality}".*expiry=[0-9]*")
#                 	if [ -z "$play_link" ]; then
#                 		printf "$c_red%s$c_reset\n" "Current video quality is not available (defaulting to highest quality)" >&2
#                 		quality=best
#                 		play_link=$(echo "$video_quality" | sort -V | tail -n 1)
#                 	fi
#                 	;;
# 	esac
#
# }
