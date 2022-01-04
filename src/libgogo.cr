#
# LIBGOGO
#   Crystal library for Gogoanime
#

#################################
#    Current Gogoanime URL      #
GOGO_URL = "https://gogoanime.cm"
#################################

# TODO: Use watzon/arachnid and kostya/lexbor instead of regexes

require "http/client"
alias HTTPCli = HTTP::Client

BASE_URL = HTTPCli.get(GOGO_URL).try &.headers["Location"].gsub("http://", "https://")

module Libgogo
  # Scrape new episodes from the homepage
  def self.new_episodes
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
  def self.search_anime(query : String)
    query = query.gsub(" ", "-")                                                       # Whitespace to dashes so we can pass it in the URL
    body = HTTP::Client.get("#{BASE_URL}/search.html?keyword=#{query}").body.to_s      # Fetch the results page
    data = {} of String => String                                                      # Initialize hash for results
    body.each_line do |line|                                                           # Iterate through each line and try to parse
      if captures = line.match(/<a href="\/category\/(?<id>.*)" title="(?<title>.*)"/) # If line can be parsed, parse it and store the captures
        data[captures["title"]] = captures["id"]                                       # Append result to hash
      end
    end
    return data # Return results
  end

  # Scrape an episode's ID from the embedded video player
  def self.get_ep_id(anime : String, episode : Int32)
    body = HTTP::Client.get("#{BASE_URL}/#{anime}-episode-#{episode}").body.to_s          # Fetch the episode's page
    ep_id = body.match(/<a href="#" rel="100".*data-video=".*(?:id([^&]+)&)/).not_nil![1] # Find episode ID
    return ep_id                                                                          # Return episode ID
  end

  # Parse episode count (range) from a series' page
  def self.ep_count(id : String) : Range
    data =
      HTTPCli.get("#{BASE_URL}/category/#{id}").body.to_s                                         # Fetch the series' page
        .match(/<a href="#" class="active" ep_start = '(\d{1,2})' ep_end = '(\d{1,5})'/).not_nil! # Pull out the episode range
      return (data[1].to_i + 1)...(data[2].to_i + 1)                                              # Return as Range
  end

  def self.get_video_url(ep_id : String, resolution : Int32 = 1080)
    data =
      HTTPCli.get("https://gogoplay1.com/download?id#{ep_id}").body.to_s                  # Fetch downloads page
        .match(/(https:\/\/.*com\/cdn.*#{resolution}p.*\?.*&amp;expiry=\d*)/).not_nil![1] # Scrape video URL from page
        .sub("&amp;", "&")                                                                # Unescape video URL
    return data
  end
end
