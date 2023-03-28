use super::*;
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::{fmt::Display, time::Duration};

const TICK_INTERVAL: Duration = Duration::from_millis(100);
lazy_static::lazy_static!(
    static ref DEFAULT_BAR_STYLE: ProgressStyle = ProgressStyle::default_bar()
        .template("{bar:40} {pos}/{len} {wide_msg}")
        .unwrap();
);

pub struct SubProgress<'a> {
    multi_progress: &'a MultiProgress,
    progress_bar: ProgressBar,
}
impl<'a> Drop for SubProgress<'a> {
    fn drop(&mut self) {
        self.progress_bar.finish();
        self.multi_progress.remove(&self.progress_bar);
    }
}
fn start_sleep<'a>(multi_progress:&'a MultiProgress, duration:std::time::Duration) -> SubProgress<'a> {
    let pb = multi_progress.add(
        ProgressBar::new_spinner().with_style(
            ProgressStyle::default_spinner()
                .template(
                    format!(
                        "{{spinner}} sleeping {{elapsed}}/{}",
                        HumanDuration(duration)
                    )
                    .as_str(),
                )
                .unwrap(),
        ),
    );
    pb.enable_steady_tick(TICK_INTERVAL);
    SubProgress {
        multi_progress: multi_progress,
        progress_bar: pb,
    }
}
impl<'a> Progress for SubProgress<'a> {
    fn suspend<F: FnOnce() -> ()>(&self, f: F) {
        self.multi_progress.suspend(f)
    }
    type SleepProg<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_sleep(&self, duration: std::time::Duration) -> Self::SleepProg<'_> {
        start_sleep(self.multi_progress, duration)
    }
}
impl<'a> FetchProg for SubProgress<'a> {
    fn set_count(&mut self, count: Option<u64>) {
        match count {
            Some(len) => {
                self.progress_bar.set_style(DEFAULT_BAR_STYLE.clone());
                self.progress_bar.set_length(len)
            }
            None => {
                self.progress_bar.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner} {pos} {wide_msg}")
                        .unwrap(),
                );
                self.progress_bar.enable_steady_tick(TICK_INTERVAL)
            }
        }
    }
    
    fn inc(&mut self, delta: u64) {
        self.progress_bar.inc(delta)
    }
}

impl<'a> ImageProg for SubProgress<'a> {
    fn set_size(&mut self, size: Option<u64>) {
        match size {
            Some(sz) => {
                self.progress_bar.set_style(
                    ProgressStyle::default_bar()
                        .template("{bar:40} {bytes}/{total_bytes}")
                        .unwrap(),
                );
                self.progress_bar.set_length(sz);
            }
            None => {
                self.progress_bar.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner} fetched {bytes}")
                        .unwrap(),
                );
                self.progress_bar.enable_steady_tick(TICK_INTERVAL);
            }
        }
    }
    fn inc(&mut self, delta: u64) {
        self.progress_bar.inc(delta)
    }
}
impl<'a> ImagesProg for SubProgress<'a> {
    type ImageRep<'b> = SubProgress<'b> 
        where Self:'a+'b;
    fn start_image<I:Display>(&mut self, url: I) -> Self::ImageRep<'_> {
        self.progress_bar.inc(1);
        self.progress_bar
            .set_message(format!("fetching image {}", url));
        Self {
            multi_progress: self.multi_progress,
            progress_bar: self.multi_progress.add(ProgressBar::hidden()),
        }
    }
    fn skip(&mut self) {
        self.progress_bar.inc(1)
    }
}

pub struct SubWrapper<'a>(pub &'a MultiProgress);

impl<'a> CommentProg for SubWrapper<'a> {
    type ChildRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_child(&self) -> Self::ChildRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self.0.add(ProgressBar::new_spinner().with_message("fetching child comments")),
        }
    }

    type ImagesRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_images(&self, count: u64) -> Self::ImagesRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self
                .0
                .add(ProgressBar::new(count).with_style(DEFAULT_BAR_STYLE.clone())),
        }
    }
}
impl<'a> CommentsProg for SubProgress<'a> {
    type CommentRep<'b> = SubWrapper<'b> where Self:'a+'b;
    fn start_comment<I: Display>(&mut self, id: I) -> Self::CommentRep<'_> {
        self.progress_bar.inc(1);
        self.progress_bar
            .set_message(format!("processing comment {}", id));
        SubWrapper(self.multi_progress)
    }
}
impl<'a> CommentTreeProg for SubWrapper<'a> {
    type FetchRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_fetch_root(&self) -> Self::FetchRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self.0.add(ProgressBar::new_spinner().with_message("getting root comments")),
        }
    }

    type FetchMissingRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_fetch_missing(&self) -> Self::FetchMissingRep<'_> {
        let pb = self
            .0
            .add(ProgressBar::new_spinner().with_message("fetching missing comments"));
        pb.enable_steady_tick(TICK_INTERVAL);
        SubProgress {
            multi_progress: self.0,
            progress_bar: pb,
        }
    }

    type CommentsRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_comments(&self, count: u64) -> Self::CommentsRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self
                .0
                .add(ProgressBar::new(count).with_style(DEFAULT_BAR_STYLE.clone())),
        }
    }
}

impl<'a> ItemProg for SubWrapper<'a> {
    type CommentTreeRep<'b> = SubWrapper<'b> where Self:'a+'b;
    fn start_comment_tree(&self) -> Self::CommentTreeRep<'_> {
        SubWrapper(self.0)
    }

    type ImagesRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_images(&self, count: u64) -> Self::ImagesRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self
                .0
                .add(ProgressBar::new(count).with_style(DEFAULT_BAR_STYLE.clone())),
        }
    }
}
impl<'a> ItemsProg for SubProgress<'a> {
    type ItemRep<'b> = SubWrapper<'b> where Self:'a+'b;
    fn start_item<I: Display>(&mut self, kind: &str, id: I) -> Self::ItemRep<'_> {
        self.progress_bar.inc(1);
        self.progress_bar
            .set_message(format!("processing {} {}", kind, id));
        SubWrapper(self.multi_progress)
    }
}

impl<'a> ItemContainerProg for SubWrapper<'a> {
    type FetchRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_fetch(&self) -> Self::FetchRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self.0.add(ProgressBar::new_spinner().with_message("fetching")),
        }
    }

    type ItemsRep<'b> = SubProgress<'b> where Self:'a+'b ;
    fn start_items(&self, count: u64) -> Self::ItemsRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self
                .0
                .add(ProgressBar::new(count).with_style(DEFAULT_BAR_STYLE.clone())),
        }
    }
}

pub struct ProgressReporter {
    multi_progress: MultiProgress,
    progress_bar: ProgressBar,
}
impl Progress for ProgressReporter {
    type SleepProg<'a> = SubProgress<'a> ;
    fn start_sleep(&self, duration: std::time::Duration) -> Self::SleepProg<'_> {
        start_sleep(&self.multi_progress, duration)
    }
    fn suspend<F: FnOnce() -> ()>(&self, f: F) {
        self.multi_progress.suspend(f)
    }
}
impl Drop for ProgressReporter {
    fn drop(&mut self) {
        self.progress_bar.finish();
        self.multi_progress.clear().unwrap();
    }
}
impl Reporter for ProgressReporter {
    fn new(jobs: Option<u64>) -> Self {
        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stdout());
        Self {
            progress_bar: match  jobs {
                Some(j) =>multi.add(ProgressBar::new(j).with_style(DEFAULT_BAR_STYLE.clone())),
                None => ProgressBar::hidden()
            } ,
            multi_progress: multi,
        }
    }

    type ItemRep<'a> = SubWrapper<'a>;
    fn start_item<I: Display>(&self, kind: &str, id: I) -> Self::ItemRep<'_> {
        self.progress_bar.inc(1);
        self.progress_bar
            .set_message(format!("processing {} {}", kind, id));
        SubWrapper(&self.multi_progress)
    }

    type ItemContainerRep<'a> = SubWrapper<'a>;
    fn start_item_container<I: Display>(
        &self,
        kind: &str,
        id: I,
        item_kind: &str,
    ) -> Self::ItemContainerRep<'_> {
        self.progress_bar.inc(1);
        self.progress_bar
            .set_message(format!("processing {} in {} {}", item_kind, kind, id));
        SubWrapper(&self.multi_progress)
    }
}
