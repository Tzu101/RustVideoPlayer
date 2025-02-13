use std::thread;
use std::time::Instant;
use std::sync::{Arc, Mutex};

use iced::widget::{image, column, button, progress_bar, row};
use iced::{stream, Background, Border, Color, Element, Gradient, Subscription};
use iced::advanced::image::Handle;
use iced::futures::{SinkExt, Stream};
use iced::gradient::Linear;

extern crate ffmpeg_next as ffmpeg;

pub fn main() -> iced::Result {
    iced::application("Crabs", Video::update, Video::view)
        .subscription(Video::subscription)
        .centered()
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    FrameDecoded(Frame),
    FrameCount(i64),
    PlaybackSpeed(f32),
    VideoFinished,
}

struct Video {
    new_frame: Option<Handle>,
    current_frame: f32,
    total_frames: f32,
    speed: Arc<Mutex<f32>>,
    playing: Arc<Mutex<bool>>,
}

impl Default for Video {
    fn default() -> Self {
        Self::new()
    }
}

impl Video {
    fn new() -> Self {
        Self {
            new_frame: None,
            current_frame: 0.0,
            total_frames: 0.0,
            speed: Arc::new(Mutex::new(1.0)),
            playing: Arc::new(Mutex::new(true)),
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::FrameDecoded(frame) => {
                self.current_frame += 1.0;
                self.new_frame = Some(Handle::from_rgba(frame.width, frame.height, frame.data));
            }
            Message::FrameCount(total_frames) => {
                self.total_frames = total_frames as f32;
            }
            Message::PlaybackSpeed(speed) => {
                *self.speed.lock().unwrap() *= speed;
            }
            Message::VideoFinished => {
                self.current_frame = 0.0;
                *self.speed.lock().unwrap() = 1.0;
            }
        }
    }

    fn view(&self) -> Element<Message> {
        if let Some(new_frame) = &self.new_frame {
            column![
                image(new_frame.clone()).width(iced::Length::Fill),
                progress_bar(0.0..=(self.total_frames - 2.0), self.current_frame).width(iced::Length::Fill).style(|_|
                    progress_bar::Style {
                        //bar: Background::Color(Color::new(1.0, 0.2, 0.2, 1.0)),
                        bar: Background::Gradient(
                            Gradient::Linear(
                                Linear::new(std::f32::consts::FRAC_PI_2)
                                .add_stop(0.0, Color::new(1.0, 0.2, 0.2, 1.0))
                                .add_stop(
                                    1.0 - 0.05 / (self.current_frame / self.total_frames),
                                    Color::new(1.0, 0.2, 0.2, 1.0))
                                .add_stop(1.0, Color::new(0.909, 0.047, 0.624, 1.0))
                            )
                        ),
                        background: Background::Color(Color::new(0.2, 0.2, 0.2, 1.0)),
                        border: Border::default(),
                    }
                ),
                row![
                    iced::widget::Space::with_width(40),
                    button("Slow down").on_press(Message::PlaybackSpeed(0.5)).style(|_, _|
                    button::Style {
                        background: Some(Background::Color(Color::new(1.0, 0.2, 0.2, 1.0))),
                        text_color: Color::WHITE,
                        ..button::Style::default()
                    }
                    ),
                    iced::widget::Space::with_width(iced::Length::Fill),
                    button("Speed up").on_press(Message::PlaybackSpeed(2.0)).style(|_, _|
                        button::Style {
                            background: Some(Background::Color(Color::new(1.0, 0.2, 0.2, 1.0))),
                            text_color: Color::WHITE,
                            ..button::Style::default()
                        }
                    ),
                    iced::widget::Space::with_width(40),
                ]
            ]
                .spacing(20)
                .into()
        } else {
            image("assets/ferris.jpg").width(iced::Length::Fill).into()
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with_id(0, Video::play(Arc::clone(&self.speed), Arc::clone(&self.playing)))
    }

    fn play(speed: Arc<Mutex<f32>>, playing: Arc<Mutex<bool>>) -> impl Stream<Item = Message> {
        stream::channel(0, move |mut output| async move {
            ffmpeg::init().unwrap();

            while *playing.lock().unwrap() {
                let (tx, rx) = std::sync::mpsc::channel();

                let speed = Arc::clone(&speed);
                thread::spawn(move || {
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
                    tx.send(Message::FrameCount(total_frames)).unwrap();

                    let frame_rate = input_stream.avg_frame_rate();
                    let frame_rate = frame_rate.0 / frame_rate.1;

                    let mut scaler = ffmpeg::software::scaling::Context::get(
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
                                    let start_time = Instant::now();

                                    let mut rgba_frame = ffmpeg::frame::Video::empty();
                                    rgba_frame.set_format(ffmpeg::format::Pixel::RGBA);
                                    rgba_frame.set_width(decoded_frame.width());
                                    rgba_frame.set_height(decoded_frame.height());

                                    scaler.run(&decoded_frame, &mut rgba_frame).unwrap();

                                    let data = rgba_frame.data(0).to_vec();
                                    let width = rgba_frame.width();
                                    let height = rgba_frame.height();

                                    tx.send(Message::FrameDecoded(Frame {
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
                });

                while let Ok(message) = rx.recv() {
                    output.send(message).await.unwrap();
                }
                output.send(Message::VideoFinished).await.unwrap();
            }
        })
    }
}

#[derive(Clone, Debug)]
struct Frame {
    data: Vec<u8>,
    width: u32,
    height: u32,
}
