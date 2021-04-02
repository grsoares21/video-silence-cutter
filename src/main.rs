use getopts::Options;
use hound::WavReader;
use indicatif::ProgressBar;
use std::cmp;
use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::process::Command;

const TEMP_DIR: &str = "./temp";

// this represents a percentage of the max volume of the video under which we'll consider the video to be silent
const SILENCE_THRESHOLD: f32 = 0.05;
// this represents how many consecutive milliseconds of noise or silence need to happen
// to consider that a noise section started or ended respctively
const ATTACK_TIME: u32 = 150;

const RELEASE_TIME: u32 = 20;

// this represents how many milliseconds each noise period will be expanded after recognized
// this setting reduces the effect of sound starting from nothing or being cut in the middle
const EXPAND: usize = 400;

enum SilenceMachineStates {
    Silence,
    PotentialNoise,
    Noise,
    PotentialSilence,
}

#[derive(Copy, Clone)]
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
    let audio_file_reader = BufReader::with_capacity(128 * 1024 * 1024, audio_file);

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
    println!("Reading audio file...");
    let mut progress_bar = ProgressBar::new(100);
    let mut read_samples = 0u32;
    let one_percent_samples = wav_reader.len() / 100;

    let samples: Vec<i32> = wav_reader
        .samples::<i32>()
        .map(|sample| {
            read_samples += 1;
            if read_samples % one_percent_samples == 0 {
                progress_bar.inc(1)
            }
            sample.unwrap()
        })
        .collect();
    progress_bar.finish_and_clear();

    println!("Finding max volume in the audio...");
    progress_bar = ProgressBar::new(100);
    read_samples = 0u32;
    let max_volume = samples
        .iter()
        .map(|sample| {
            read_samples += 1;
            if read_samples % one_percent_samples == 0 {
                progress_bar.inc(1)
            }
            sample.abs()
        })
        .max()
        .unwrap();
    println!("Max volume: {}", max_volume);

    // create chunks of 1ms and store the max volume in each chunk
    let mut max_volume_chunks: Vec<i32> = Vec::new();
    for i in 0..samples.len() / samples_per_millisecond as usize {
        let chunk_initial_offset = i * samples_per_millisecond as usize;
        let chunk_final_offset =
            cmp::min((i + 1) * samples_per_millisecond as usize, samples.len());

        let chunk: &[i32] = &samples[chunk_initial_offset..chunk_final_offset];

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

    let silence_threshold = (max_volume as f32 * SILENCE_THRESHOLD) as i32;

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

    println!("Expanding noise sections by {}ms", EXPAND);

    let mut expanded_noise_sections: Vec<Section> = Vec::new();

    let mut current_section = Section {
        from: if EXPAND > noise_sections[0].from {
            0
        } else {
            noise_sections[0].from - EXPAND
        },
        to: cmp::min(noise_sections[0].to + EXPAND, max_volume_chunks.len()),
    };

    for section in &noise_sections[1..] {
        if current_section.to >= section.from - EXPAND {
            current_section.to = section.to + EXPAND;
        } else {
            expanded_noise_sections.push(current_section);
            current_section = Section {
                from: section.from - EXPAND,
                to: section.to + EXPAND,
            }
        }
    }
    expanded_noise_sections.push(current_section);

    println!(
        "Number of recognized sections: {}, Number of sections after expanding: {}",
        noise_sections.len(),
        expanded_noise_sections.len()
    );
    println!("Resulting expanded sections:");
    for section in &expanded_noise_sections {
        println!("From {}ms to {}ms", section.from, section.to);
    }

    let time_filter = expanded_noise_sections
        .iter()
        .map(|section| {
            format!(
                "between(t,{},{})",
                section.from as f32 / 1000f32,
                section.to as f32 / 1000f32
            )
        })
        .collect::<Vec<String>>()
        .join("+");

    let mut audio_filter_file = File::create("./temp/audio_filter.txt")?;
    let mut video_filter_file = File::create("./temp/video_filter.txt")?;

    write!(
        video_filter_file,
        "select='{}', setpts=N/FRAME_RATE/TB",
        time_filter
    )?;
    write!(
        audio_filter_file,
        "aselect='{}', asetpts=N/SR/TB",
        time_filter
    )?;

    let mut cut_video_process = Command::new("ffmpeg")
        .args(&["-i", &input_file_name])
        .args(&["-filter_script:v", "./temp/video_filter.txt"])
        .args(&["-filter_script:a", "./temp/audio_filter.txt"])
        .arg(output_file_name)
        .spawn()
        .expect("Failed to spawn process to cut video");

    cut_video_process
        .wait()
        .expect("Failed to wait for process to cut video");

    Ok(())
}
