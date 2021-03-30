use getopts::Options;
use hound::WavReader;
use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

const TEMP_DIR: &str = "./temp";

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
            panic!(error.to_string())
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

    let wav_reader = WavReader::new(audio_file_reader);

    let spec = wav_reader.unwrap().spec();

    println!(
        "Channels: {}\nBits per sample: {}\nSample rate:{}",
        spec.channels, spec.bits_per_sample, spec.sample_rate
    );

    Ok(())
}
