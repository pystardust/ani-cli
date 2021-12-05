# How to contribute
- Keep the script POSIX sh compatible.
- Mention extra dependencies if used
- Try using builtin and minimize external commands

## Adding a new source
- A source's name and url is added to the sources object at the top of the file.
- All new sources that are added require the following functions listed below for them to integrate correctly. These functions are declared under a function that is named after the source name. See current sources for clarification on this.

### `search_anime`
Passed the search result made by the user.  
Returns a json object which contains the sites internal id to the show, and the animes display name.  
This is so the details can be easily parsed by jq for use elsewhere in the program.
- The id is used in navigating to the web page of the show
- The display name is shown in the selection menu
- Example object `{"id": "aaiutsa5", "name": "86 (2021)"}`

### `get_episode_count`
Passed the id of the anime selected by the user.  
Returns the total episode count for the selected anime.
- The id is used in fetching the episode count from the site
- Example response `2`

### `get_video_url`
Passed the anime id and episode number selected by the user.  
Returns an `video_content` object to be used by `play_video` and `download_video`. This should contain all the needed details.
- Example object `{"video_url": "https://...", "embed_url": "https://..."}`

### `play_video`
Passed the `video_content` object.  
Plays the video file from the details in the `video_content` object.

### `download_video`
Passed the `video_content` object.  
Downloads the video file from the details in the `video_content` object.