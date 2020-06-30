import { exec } from "child_process";
import * as fs from "fs";
import silenceMachineCreator from "./silenceMachine";

const silenceThreshold = 0.075;
const attackTime = 30;
const releaseTime = 10;

//TODO: create temp folder if it doesn't exist, it if does, raise an error
/*exec("ffmpeg -i raw.mp4 -ab 160k -ac 2 -vn temp/audio.wav", (error, stdout, stderr) => {
  if (error) {
    console.error(`error: ${error.message}`);
    return;
  }
  if (stderr) {
    console.error(`${stderr}`);
    return;
  }
  console.log(`${stdout}`);
});*/

type Section = { from: number; to?: number };

const readStream = fs.createReadStream("temp/audio.wav");
const data: Buffer[] = [];

console.log("Reading audio file");
readStream.on("data", (chunk) => {
  data.push(chunk as Buffer);
  // data : <Buffer 49 20 61 6d 20 74 72 61 6e 73 66 65 72 72 69 6e> 16
  // data : <Buffer 67 20 69 6e 20 62 79 74 65 73 20 62 79 20 62 79> 16
  // data : <Buffer 74 65 73 20 63 61 6c 6c 65 64 20 63 68 75 6e 6b> 16
});

readStream.on("end", () => {
  console.log("Audio file read.");
  let fileData = Buffer.concat(data);
  const chunkSize = fileData.readInt32LE(4);
  const format = readBytesAsText(fileData, 8, 4);

  const subChunk1Id = readBytesAsText(fileData, 12, 4);

  console.log(subChunk1Id);

  const subChunk1Size = fileData.readInt32LE(16);
  const audioFormat = fileData.readInt16LE(20);
  const channelNumbers = fileData.readInt16LE(22);
  const sampleRate = fileData.readInt32LE(24);
  const byteRate = fileData.readInt32LE(28);
  const bitsPerSample = fileData.readInt16LE(34);

  let byteToSeconds = (bytes: number): number => {
    return Math.round(bytes / byteRate);
  };

  let byteToMilisseconds = (bytes: number): number => {
    return Math.round(bytes / (byteRate / 1000));
  };

  console.log(`
    sub chunk 1 size: ${subChunk1Size}
    audioFormat: ${audioFormat}
    channel numbers: ${channelNumbers}
    sample rate: ${sampleRate}
    byteRate: ${byteRate}
    bits per sample: ${bitsPerSample}
    byte per sample: ${bitsPerSample / 8}`);

  const subChunk2Id = readBytesAsText(fileData, 36, 4);

  const subChunk2Size = fileData.readInt32LE(40);
  const listTypeId = readBytesAsText(fileData, 44, 4);
  let listId = readBytesAsText(fileData, 48, 4);
  const listTextSize = fileData.readInt32LE(52);
  let listText = readBytesAsText(fileData, 56, listTextSize);
  console.log(`
    Sub chunk 2 id: ${subChunk2Id}
    sub chunk 2 size: ${subChunk2Size}
    list type id: ${listTypeId}
    list info: ${listId}
    list size: ${listTextSize}
    list text: ${listText}`);

  const subChunk3Id = readBytesAsText(fileData, 70, 4);
  const subChunk3Size = fileData.readInt32LE(74);

  console.log(`
    sub chunk 3 id: ${subChunk3Id}
    sub chunk 3 size: ${subChunk3Size}`);

  let maxvolume = getMaxVolume(fileData, 78);
  console.log(
    `Max volume: ${maxvolume.maxVolume} Byte offset: ${maxvolume.offset} No Of seconds: ${byteToSeconds(
      maxvolume.offset
    )}`
  );

  const silenceMachine = silenceMachineCreator(attackTime, releaseTime);

  let previousState = null;

  // Discretize volumes in chunks of 352 bytes (~2ms)
  const chunksMaxVolume = [];
  const totalChunksInVideo = Math.ceil(subChunk3Size / 352);
  let chunk;
  for (chunk = 0; chunk < totalChunksInVideo; chunk++) {
    let maxVolumeInChunk = 0;
    for (
      let byteOffsetInChunk = 0;
      byteOffsetInChunk < 352 && chunk * 352 + byteOffsetInChunk < subChunk3Size;
      byteOffsetInChunk += 2
    ) {
      let byteOffset = 78 + chunk * 352 + byteOffsetInChunk;

      let currentSample = fileData.readInt16LE(byteOffset);
      if (Math.abs(currentSample) > maxVolumeInChunk) {
        maxVolumeInChunk = Math.abs(currentSample);
      }
    }
    chunksMaxVolume.push(maxVolumeInChunk);
  }

  console.log("Number of chunks: " + chunksMaxVolume.length);

  let i = 0;

  let noiseSections: Section[] = [];

  let section: Section;
  // Uses statemachine transitions to store silence sections
  silenceMachine.onTransition((state) => {
    if (state.changed && state.value !== previousState) {
      let ms = byteToMilisseconds(i * 352 + 78);
      if (previousState === "PotentialSilenceFinish" && state.value === "Noisy") {
        section = { from: ms, to: null };
        console.log(`At ${ms}ms: previous - ${previousState} current - ${state.value}`);
      } else if (previousState === "PotentialSilenceStart" && state.value === "Silence") {
        section.to = ms;
        noiseSections.push(section);
        console.log(`At ${ms}ms: previous - ${previousState} current - ${state.value}`);
      }

      previousState = state.value;
    }
  });

  // Run chunks through state machine
  silenceMachine.start();
  for (; i < chunksMaxVolume.length; i++) {
    let currentChunk = chunksMaxVolume[i];
    if (currentChunk < maxvolume.maxVolume * silenceThreshold) {
      silenceMachine.send("SAMPLE_SILENCE");
    } else {
      silenceMachine.send("SAMPLE_NOISY");
    }
  }

  console.log(`Number of noise sections in video: ${noiseSections.length}`);
});

function readBytesAsText(buffer: Buffer, offset: number, size: number): string {
  let text = "";
  for (let i = offset; i < offset + size; i++) {
    text += String.fromCharCode(buffer.readInt8(i));
  }

  return text;
}

function getMaxVolume(audioData: Buffer, initialOffset: number): { offset: number; maxVolume: number } {
  let maxVolume = Number.NEGATIVE_INFINITY;
  let offset = 0;
  for (let i = initialOffset; i < audioData.length; i += 2) {
    let currentSample = audioData.readInt16LE(i);
    if (Math.abs(currentSample) > maxVolume) {
      maxVolume = currentSample;
      offset = i;
    }
  }

  return { maxVolume, offset };
}

readStream.on("error", (err) => {
  console.log("error :", err);
});
