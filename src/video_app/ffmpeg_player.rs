extern crate ffmpeg_next as ffmpeg;

use std::{thread, time};
use std::sync::{Arc, Mutex, mpsc};

#[derive(Debug, Clone)]
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub enum Signal {
    DecodedFrame(Frame),
    TotalFrames(i64),
}

pub fn play(sender: mpsc::Sender<Signal>, speed: Arc<Mutex<f32>>) {
    let mut input_context = if let Ok(ctx) = ffmpeg_next::format::input("assets/crab_rave.mp4") {
        ctx
    } else {
        return;
    };

    let input_stream = input_context.streams().best(ffmpeg::media::Type::Video).unwrap();
    let video_stream_index = input_stream.index();

    let decoder_context = ffmpeg_next::codec::Context::from_parameters(input_stream.parameters()).unwrap();
    let mut packet_decoder = decoder_context.decoder().video().unwrap();

    let total_frames = input_stream.frames();
    sender.send(Signal::TotalFrames(total_frames)).unwrap();

    let frame_rate = input_stream.avg_frame_rate();
    let frame_rate = frame_rate.0 / frame_rate.1;

    let mut scalar = ffmpeg::software::scaling::Context::get(
        packet_decoder.format(),
        packet_decoder.width(),
        packet_decoder.height(),
        ffmpeg::format::Pixel::RGBA,
        packet_decoder.width(),
        packet_decoder.height(),
        ffmpeg::software::scaling::Flags::BILINEAR,
    ).unwrap();

    for (stream, packet) in input_context.packets() {
        if stream.index() == video_stream_index {
            if packet_decoder.send_packet(&packet).is_ok() {
                let mut decoded_frame = ffmpeg::frame::Video::empty();

                while packet_decoder.receive_frame(&mut decoded_frame).is_ok() {
                    let start_time = time::Instant::now();

                    let mut rgba_frame = ffmpeg::frame::Video::empty();
                    rgba_frame.set_format(ffmpeg::format::Pixel::RGBA);
                    rgba_frame.set_width(decoded_frame.width());
                    rgba_frame.set_height(decoded_frame.height());

                    scalar.run(&decoded_frame, &mut rgba_frame).unwrap();

                    let data = rgba_frame.data(0).to_vec();
                    let width = rgba_frame.width();
                    let height = rgba_frame.height();

                    sender.send(Signal::DecodedFrame(Frame {
                        data,
                        width,
                        height,
                    })).unwrap();

                    let fps = (frame_rate as f32 * *speed.lock().unwrap()) as i32;

                    let elapsed_time = start_time.elapsed().as_millis();
                    let mut time_per_frame = 1000 / fps - elapsed_time as i32;
                    if time_per_frame < 0 {
                        time_per_frame = 0;
                    }
                    thread::sleep(std::time::Duration::from_millis(time_per_frame as u64));
                }
            }
        }
    }
}