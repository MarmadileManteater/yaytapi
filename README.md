
# yet another yt api

This server provides an [Invidious](https://github.com/iv-org/invidious)-like output in order to act as an API for applications which utilize the Invidious API such as [FreeTube](https://github.com/FreeTubeApp/FreeTube).

This uses [yayti](https://github.com/MarmadileManteater/yayti) for extraction and formatting of data from YT.

## 🛠 Arguments
- `--use-android-endpoint` 
  - Enables use of the android client info when accessing the `/player` endpoint
  - Streams do not need to be deciphered from the android endpoint
- `--decipher-streams`
  - Uses [`boa_engine`](https://github.com/boa-dev/boa) to decipher streams
  - _(defaults to deciphering on demand)_
- `--pre-decipher-streams`
  - Pre-deciphers every stream in an output _(⚠severe performance penalty on some videos endpoint calls)_
- `--mongo-db=connection_string` 
  - Sets the db preference to [MongoDb](https://www.mongodb.com/), and uses the connection string to connect to it. 
  - _(default db preference is [UnQLite](https://unqlite.org/))_
- `--no-cache`
  - Disables caching innertube responses
- `--return-innertube`
  - Returns entire innertube response _(useful for 🐛debugging, but unuseful outside of that)_
- `--enable-local-streaming`
  - Allows proxying of videos through server
- `--enable-cors`
  - Enables a permissive CORS policy
- `--ip=127.0.0.1`
- `--port=8080`


## 👩‍🏭 progress
- [X] `/api/v1/stats`
- [X] `/api/v1/videos` (missing certain aspects such as storyboards, but works for non-edge cases)
- [ ] `/api/manifest/dash/id/{video_id}`
- [ ] `/api/v1/comments`
- [ ] `/api/v1/trending`
- [ ] `/api/v1/channels`
  - [ ] `/api/v1/channels/comments/{author_id}`
  - [ ] `/api/v1/channels/search/{author_id}`
- [ ] `/api/v1/search`
- [ ] `/api/v1/playlists`
  - [X] working `page` parameter 
- [ ] `/api/v1/mixes`
- [ ] `/api/v1/captions` (unimplemented, but direct links to vtt are passed through)
- [ ] `/api/v1/storyboards`
- [X] `/vi/{video_id}/{file_name}.jpg`
- [X] `/ggpht/{author_thumbnail}`
- [X] `/latest_version`
- [X] `/videoplayback`
- [X] `/decipher_streams` (not an invidious endpoint, used for deciphering when enabled)

