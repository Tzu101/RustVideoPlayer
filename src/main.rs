mod video_app;
use video_app::Video;

pub fn main() -> iced::Result {
    iced::application("Crabs", Video::update, Video::view)
        .subscription(Video::subscription)
        .centered()
        .run()
}

