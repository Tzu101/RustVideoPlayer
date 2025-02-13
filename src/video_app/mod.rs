use std::thread;
use std::time::Instant;
use std::sync::{Arc, Mutex};

use iced::widget::{image, column, button, progress_bar, row};
use iced::{stream, Background, Border, Color, Element, Gradient, Subscription};
use iced::advanced::image::Handle;
use iced::gradient::Linear;
use iced::futures::{SinkExt, Stream};

mod ffmpeg_player;
extern crate ffmpeg_next as ffmpeg;


#[derive(Debug, Clone)]
pub enum Message {
    NewFrame(ffmpeg_player::Frame),
    FrameCount(i64),
    PlaybackSpeed(f32),
    VideoFinished,
}

pub struct Video {
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

    pub fn update(&mut self, message: Message) {
        match message {
            Message::NewFrame(frame) => {
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

    pub fn view(&self) -> Element<Message> {
        if let Some(new_frame) = &self.new_frame {
            iced::widget::column![
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

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with_id(0, Video::play(Arc::clone(&self.speed), Arc::clone(&self.playing)))
    }

    fn play(speed: Arc<Mutex<f32>>, playing: Arc<Mutex<bool>>) -> impl Stream<Item = Message> {
        stream::channel(0, move |mut output| async move {
            ffmpeg::init().unwrap();

            while *playing.lock().unwrap() {
                let (tx, rx) = std::sync::mpsc::channel();

                let speed = Arc::clone(&speed);
                thread::spawn(move || {
                    ffmpeg_player::play(tx, speed);
                });

                while let Ok(message) = rx.recv() {
                    match message {
                        ffmpeg_player::Signal::DecodedFrame(frame) => {
                            output.send(Message::NewFrame(frame)).await.unwrap();
                        }
                        ffmpeg_player::Signal::TotalFrames(total_frames) => {
                            output.send(Message::FrameCount(total_frames)).await.unwrap();
                        }
                    }
                }
                output.send(Message::VideoFinished).await.unwrap();
            }
        })
    }
}
