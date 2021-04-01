use getopts::Options;
use hound::WavReader;
use std::cmp;
use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

const TEMP_DIR: &str = "./temp";

// this represents a percentage of the max volume of the video under which we'll consider the video to be silent
const SILENCE_THRESHOLD: f32 = 0.05;
// this represents how many consecutive milliseconds of noise or silence need to happen
// to consider that a noise section started or ended respctively
const ATTACK_TIME: u32 = 150;

const RELEASE_TIME: u32 = 20;

enum SilenceMachineStates {
    Silence,
    PotentialNoise,
    Noise,
    PotentialSilence,
}

struct Section {
    from: usize,
    to: usize,
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut opts = Options::new();

    opts.optopt(
        "i",
        "input",
        "Set input file",
        "Indicate the video file to cut the sileces from",
    );
    opts.optopt(
        "o",
        "output",
        "Set output file",
        "Indicate the path of the output video file",
    );

    // in Rust the first argument is the program itself and the following ones are the real args
    let matches = match opts.parse(&args[1..]) {
        Ok(matches) => matches,
        Err(error) => {
            panic!("{}", error.to_string())
        }
    };

    let input_file_name = match matches.opt_str("i") {
        Some(file_name) => file_name,
        None => panic!("You need to pass an input file (-i option)"),
    };
    let output_file_name = match matches.opt_str("o") {
        Some(file_name) => file_name,
        None => panic!("You need to pass an output file (-o option)"),
    };

    if Path::new(TEMP_DIR).is_dir() {
        println!(
            "Found existing {} directory, deleting and recreating it",
            TEMP_DIR
        );
        fs::remove_dir_all(TEMP_DIR)?;
    }

    fs::create_dir(TEMP_DIR)?;

    println!(
        "Input file name: {}, output file name: {}",
        input_file_name, output_file_name
    );

    let mut split_audio_process = Command::new("ffmpeg")
        .args(&["-i", &input_file_name])
        .args(&["-ab", "160k"])
        .args(&["-ac", "2"])
        .args(&["-vn", "temp/audio.wav"])
        .spawn()
        .expect("Failed to spawn process to separate video from audio");

    split_audio_process
        .wait()
        .expect("Failed to wait for process to separate video from audio");

    let audio_file = File::open("temp/audio.wav")?;
    let audio_file_reader = BufReader::new(audio_file);

    let mut wav_reader = WavReader::new(audio_file_reader).unwrap();

    let spec = wav_reader.spec();

    println!(
        "Channels: {}\nBits per sample: {}\nSample rate:{}",
        spec.channels, spec.bits_per_sample, spec.sample_rate
    );

    // sample_rate is in samples per second per channel
    let samples_per_millisecond = (spec.sample_rate as u32 / 1000) * spec.channels as u32;

    // we know that ffmpeg will produce a 16 bit sample, otherwise we'd need to check the bits per sample to choose the correct type
    // read all samples of the file an put in a vectore (no need for a buffer here)
    let samples: Vec<i16> = wav_reader
        .samples::<i16>()
        .map(|sample| sample.unwrap())
        .collect();

    let max_volume = samples.iter().map(|sample| sample.abs()).max().unwrap();
    println!("Max volume: {}", max_volume);

    // create chunks of 1ms and store the max volume in each chunk
    let mut max_volume_chunks: Vec<i16> = Vec::new();
    for i in 0..samples.len() / samples_per_millisecond as usize {
        let chunk_initial_offset = i * samples_per_millisecond as usize;
        let chunk_final_offset =
            cmp::min((i + 1) * samples_per_millisecond as usize, samples.len());

        let chunk: &[i16] = &samples[chunk_initial_offset..chunk_final_offset];

        let max_volume_in_chunk = chunk.iter().map(|sample| sample.abs()).max().unwrap();
        max_volume_chunks.push(max_volume_in_chunk);
    }

    println!(
        "Number of chunks (number of millisseconds): {}",
        max_volume_chunks.len()
    );

    let mut current_state = SilenceMachineStates::Silence;
    let mut consecutive_silence_chunks = 0;
    let mut consecutive_noise_chunks = 0;

    let silence_threshold = (max_volume as f32 * SILENCE_THRESHOLD) as i16;

    let mut noise_sections: Vec<Section> = Vec::new();

    let mut beginning_of_noise: usize = 0;

    for i in 0..max_volume_chunks.len() {
        if max_volume_chunks[i] < silence_threshold {
            // silent chunk
            match current_state {
                SilenceMachineStates::Silence => {}
                SilenceMachineStates::Noise => {
                    current_state = SilenceMachineStates::PotentialSilence;
                    consecutive_silence_chunks = 1;
                }
                SilenceMachineStates::PotentialSilence => {
                    consecutive_silence_chunks += 1;
                    if consecutive_silence_chunks > ATTACK_TIME {
                        noise_sections.push(Section {
                            from: beginning_of_noise,
                            to: i,
                        });
                        println!(
                            "Noise section identified from: {}ms to {}ms",
                            beginning_of_noise, i
                        );
                        current_state = SilenceMachineStates::Silence;
                    }
                }
                SilenceMachineStates::PotentialNoise => {
                    current_state = SilenceMachineStates::Silence;
                }
            }
        } else {
            // noisy chunk
            match current_state {
                SilenceMachineStates::Silence => {
                    current_state = SilenceMachineStates::PotentialNoise;
                    consecutive_noise_chunks = 1;
                }
                SilenceMachineStates::Noise => {}
                SilenceMachineStates::PotentialSilence => {
                    current_state = SilenceMachineStates::Noise;
                }
                SilenceMachineStates::PotentialNoise => {
                    consecutive_noise_chunks += 1;
                    if consecutive_noise_chunks > RELEASE_TIME {
                        beginning_of_noise = i;
                        current_state = SilenceMachineStates::Noise;
                    }
                }
            }
        }
    }

    Ok(())
}
