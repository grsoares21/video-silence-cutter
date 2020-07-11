# Video Silence Cutter
A NodeJS script written in TypeScript to trim quiet sections out of videos.

## Requirements
This script uses [FFmpeg](https://ffmpeg.org/) for its manipulations on video and audio files.
You need to have FFmpeg installed in your machine for it to work.
The script requires Node version 12.0.0 or greater.

## Installing
Clone the repository and install all Node dependencies before running it.
```
npm install
```

## Running
After installing all dependencies, build the project by running the following command:
```
npm run build
```
You can run it either by directly running the built script:
```
node build/index.js -i input.mp4
```
Or you can run it via the `start` npm task:
```
npm run start -- -i input.mp4
```
### Options
| Option                | Type          | Meaning                                                     | Default    |
|-----------------------|---------------|-------------------------------------------------------------|------------|
| `-i` or `--input`     | `string`      | Path to the input video file                                | N/A        |
| `-o` or `--output`    | `string`      | Name of the output file (with extension)                    | output.mp4 |

Example:
```
npm run start -- -i input.mp4 -o output.mp4
```

## License
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Credits
Made with <3 by Gabriel R. Soares

[Github](https://github.com/grsoares21/)

[Twitter](https://twitter.com/_grsoares)

[Twitch](https://www.twitch.tv/grsoares)

[YouTube](https://www.youtube.com/playlist?list=PL0uQGewjvqzr7gC1Rr1w4nJ2Cqd9w2r8j)
