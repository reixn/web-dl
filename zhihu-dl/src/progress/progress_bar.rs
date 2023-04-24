use super::*;
use indicatif::{HumanDuration, MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::{
    fmt::Display,
    time::{Duration, SystemTime},
};
use yansi::Paint;

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
async fn start_sleep(multi_progress: &MultiProgress, duration: std::time::Duration) {
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
    tokio::time::sleep(duration).await;
    multi_progress.remove(&pb);
}
impl<'a> Progress for SubProgress<'a> {
    async fn sleep(&self, duration: std::time::Duration) {
        start_sleep(self.multi_progress, duration).await
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
    fn start_image<I: Display>(&mut self, url: I) -> Self::ImageRep<'_> {
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

impl<'a> Progress for SubWrapper<'a> {
    async fn sleep(&self, duration: std::time::Duration) {
        start_sleep(self.0, duration).await;
    }
}

impl<'a> CommentProg for SubWrapper<'a> {
    type ChildRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_child(&self) -> Self::ChildRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self
                .0
                .add(ProgressBar::new_spinner().with_message("fetching child comments")),
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
    fn skip_comment(&mut self) {
        self.progress_bar.inc(1);
        self.progress_bar.set_message("processing comment");
    }
}
impl<'a> CommentTreeProg for SubWrapper<'a> {
    type FetchRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_fetch_root(&self) -> Self::FetchRep<'_> {
        SubProgress {
            multi_progress: self.0,
            progress_bar: self
                .0
                .add(ProgressBar::new_spinner().with_message("getting root comments")),
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

fn item_start_images(prog: &MultiProgress, count: u64) -> SubProgress<'_> {
    SubProgress {
        multi_progress: prog,
        progress_bar: prog.add(ProgressBar::new(count).with_style(DEFAULT_BAR_STYLE.clone())),
    }
}
impl<'a> ItemProg for SubWrapper<'a> {
    type CommentTreeRep<'b> = SubWrapper<'b> where Self:'a+'b;
    fn start_comment_tree(&self) -> Self::CommentTreeRep<'_> {
        SubWrapper(self.0)
    }

    type ImagesRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_images(&self, count: u64) -> Self::ImagesRep<'_> {
        item_start_images(self.0, count)
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
    fn skip_item(&mut self) {
        self.progress_bar.inc(1);
        self.progress_bar.set_message("processing");
    }
}

fn container_start_fetch(prog: &MultiProgress) -> SubProgress<'_> {
    SubProgress {
        multi_progress: prog,
        progress_bar: prog.add(ProgressBar::new_spinner().with_message("fetching")),
    }
}
fn container_start_items(prog: &MultiProgress, count: u64) -> SubProgress<'_> {
    SubProgress {
        multi_progress: prog,
        progress_bar: prog.add(ProgressBar::new(count).with_style(DEFAULT_BAR_STYLE.clone())),
    }
}
impl<'a> ItemContainerProg for SubWrapper<'a> {
    type FetchRep<'b> = SubProgress<'b> where Self:'a+'b;
    fn start_fetch(&self) -> Self::FetchRep<'_> {
        container_start_fetch(self.0)
    }
    type ItemsRep<'b> = SubProgress<'b> where Self:'a+'b ;
    fn start_items(&self, count: u64) -> Self::ItemsRep<'_> {
        container_start_items(self.0, count)
    }
}

pub struct ProgressReporter {
    pub multi_progress: MultiProgress,
    progress_bar: ProgressBar,
}
impl Progress for ProgressReporter {
    async fn sleep(&self, duration: std::time::Duration) {
        start_sleep(&self.multi_progress, duration).await
    }
}
impl Drop for ProgressReporter {
    fn drop(&mut self) {
        self.progress_bar.finish();
        self.multi_progress.clear().unwrap();
    }
}

pub struct Item<'a> {
    multi_progress: &'a MultiProgress,
    start_time: SystemTime,
    prefix: &'static str,
    kind: &'static str,
    option: String,
}
impl<'a> Item<'a> {
    fn new<I: Display, O: Display>(
        multi_progress: &'a MultiProgress,
        operation: &str,
        prefix: &'static str,
        kind: &'static str,
        id: I,
        option: Option<O>,
    ) -> Self {
        let option = match option {
            Some(opt) => format!("({})", opt),
            None => String::default(),
        };
        multi_progress.suspend(|| {
            println!(
                "{op:>13} {prefix}{kind} {id} {option}",
                op = Paint::cyan(operation),
                prefix = prefix,
                kind = kind,
                id = id,
                option = option
            )
        });
        Self {
            multi_progress,
            start_time: SystemTime::now(),
            prefix,
            kind,
            option,
        }
    }
    fn link<I: Display, P: AsRef<Path>>(
        multi_progress: &'a MultiProgress,
        kind: &str,
        id: I,
        dest: P,
    ) {
        multi_progress.suspend(|| {
            println!(
                "{op:>13} {kind} {id} to {dest}",
                op = Paint::green("Linked"),
                kind = kind,
                id = id,
                dest = dest.as_ref().display()
            )
        })
    }
}
impl<'a> Progress for Item<'a> {
    async fn sleep(&self, duration: std::time::Duration) {
        start_sleep(self.multi_progress, duration).await
    }
}
impl<'a> ItemProg for Item<'a> {
    type CommentTreeRep<'b> = SubWrapper<'b>
        where Self:'a+'b;
    fn start_comment_tree(&self) -> Self::CommentTreeRep<'_> {
        SubWrapper(self.multi_progress)
    }
    type ImagesRep<'b> = SubProgress<'b>
        where Self:'a+'b;
    fn start_images(&self, count: u64) -> Self::ImagesRep<'_> {
        item_start_images(self.multi_progress, count)
    }
}
impl<'a> ItemJob for Item<'a> {
    fn finish<I: Display>(self, operation: &str, id: I) {
        self.multi_progress.suspend(|| {
            println!(
                "{op:>13} {prefix}{kind} {id} {opt} took {dur}",
                op = Paint::green(operation),
                prefix = self.prefix,
                kind = self.kind,
                id = id,
                opt = self.option,
                dur = HumanDuration(SystemTime::now().duration_since(self.start_time).unwrap())
            )
        });
    }
}

pub struct Container<'a> {
    multi_progress: &'a MultiProgress,
    start_time: SystemTime,
    prefix: &'static str,
    kind: &'static str,
    item_kind: &'static str,
    option_name: &'static str,
    option: String,
}
impl<'a> Progress for Container<'a> {
    async fn sleep(&self, duration: std::time::Duration) {
        start_sleep(self.multi_progress, duration).await
    }
}
impl<'a> Container<'a> {
    fn new<II, IO, IC, I, O>(
        multi_progress: &'a MultiProgress,
        operation: &str,
        prefix: &'static str,
        id: I,
        option: Option<O>,
    ) -> Self
    where
        II: item::Item,
        IC: item::ItemContainer<IO, II>,
        I: Display,
        O: Display,
    {
        let option = match option {
            Some(opt) => format!("({})", opt),
            None => String::default(),
        };
        multi_progress.suspend(|| {
            println!(
                "{op:>13} {prefix}{item_kind} ({item_opt}) in {kind} {id} {option}",
                op = Paint::cyan(operation),
                prefix = prefix,
                item_kind = II::TYPE,
                item_opt = IC::OPTION_NAME,
                kind = IC::TYPE,
                id = id,
                option = option
            )
        });
        Self {
            multi_progress,
            start_time: SystemTime::now(),
            prefix,
            kind: IC::TYPE,
            item_kind: II::TYPE,
            option_name: IC::OPTION_NAME,
            option,
        }
    }
    fn link<II, IO, IC, I, P>(multi_progress: &'a MultiProgress, id: I, dest: P)
    where
        II: item::Item,
        IC: item::ItemContainer<IO, II>,
        I: Display,
        P: AsRef<Path>,
    {
        multi_progress.suspend(|| {
            println!(
                "{op:>13} {item_kind} ({option}) in {kind} {id} to {dest}",
                op = Paint::green("Linked"),
                item_kind = II::TYPE,
                option = IC::OPTION_NAME,
                kind = IC::TYPE,
                id = id,
                dest = dest.as_ref().display()
            )
        });
    }
}
impl<'a> ItemContainerProg for Container<'a> {
    type FetchRep<'b> = SubProgress<'b>
        where Self:'a+'b ;
    fn start_fetch(&self) -> Self::FetchRep<'_> {
        container_start_fetch(self.multi_progress)
    }
    type ItemsRep<'b> = SubProgress<'b>
        where Self:'a+'b;
    fn start_items(&self, count: u64) -> Self::ItemsRep<'_> {
        container_start_items(self.multi_progress, count)
    }
}
impl<'a> ContainerJob for Container<'a> {
    fn finish<I: Display>(self, operation: &str, num: Option<usize>, id: I) {
        self.multi_progress.suspend(|| {
            println!(
                "{op:>13} {prefix}{num}{item_kind} ({item_opt}) in {kind} {id} {opt} took {dur}",
                op = Paint::green(operation),
                prefix = self.prefix,
                num = match num {
                    Some(v) => format!("{} ", v),
                    None => String::new(),
                },
                item_kind = self.item_kind,
                item_opt = self.option_name,
                kind = self.kind,
                id = id,
                opt = self.option,
                dur = HumanDuration(SystemTime::now().duration_since(self.start_time).unwrap())
            )
        });
    }
}

pub struct Job<'a> {
    multi_progress: &'a MultiProgress,
    start_time: SystemTime,
}
impl<'a> Job<'a> {
    fn new<I: Display>(multi_progress: &'a MultiProgress, operation: &str, msg: I) -> Self {
        multi_progress.suspend(|| println!("{:>13} {}", Paint::cyan(operation), msg));
        Self {
            multi_progress,
            start_time: SystemTime::now(),
        }
    }
}
impl<'a> OtherJob for Job<'a> {
    fn finish<I: Display>(self, operation: &str, msg: I) {
        self.multi_progress.suspend(|| {
            println!(
                "{:>13} {} took {}",
                Paint::green(operation),
                msg,
                HumanDuration(SystemTime::now().duration_since(self.start_time).unwrap())
            )
        })
    }
}

impl<'b> Reporter for Container<'b> {
    type ItemRep<'a> = Item<'a> where Self:'a;
    fn start_item<O: Display, I: Display>(
        &self,
        operation: &str,
        prefix: &'static str,
        kind: &'static str,
        id: I,
        option: Option<O>,
    ) -> Self::ItemRep<'_> {
        Item::new(self.multi_progress, operation, prefix, kind, id, option)
    }
    fn link_item<I: Display, P: AsRef<Path>>(&self, kind: &str, id: I, dest: P) {
        Item::link(self.multi_progress, kind, id, dest)
    }

    type ItemContainerRep<'a> = Container<'a> where Self:'a;
    fn start_item_container<II, IO, IC, I, O>(
        &self,
        operation: &str,
        prefix: &'static str,
        id: I,
        option: Option<O>,
    ) -> Self::ItemContainerRep<'_>
    where
        II: item::Item,
        IC: item::ItemContainer<IO, II>,
        I: Display,
        O: Display,
    {
        Container::new::<II, IO, IC, _, _>(self.multi_progress, operation, prefix, id, option)
    }
    fn link_container<II, IO, IC, I, P>(&self, id: I, dest: P)
    where
        II: item::Item,
        IC: item::ItemContainer<IO, II>,
        I: Display,
        P: AsRef<Path>,
    {
        Container::link::<II, IO, IC, _, _>(self.multi_progress, id, dest)
    }

    type JobRep<'a> = Job<'a> where Self:'a;
    fn start_job<I: Display>(&self, operation: &str, msg: I) -> Self::JobRep<'_> {
        Job::new(self.multi_progress, operation, msg)
    }
}

impl ProgressReporter {
    pub fn new(jobs: Option<u64>) -> Self {
        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stdout());
        Self {
            progress_bar: match jobs {
                Some(j) => multi.add(ProgressBar::new(j).with_style(DEFAULT_BAR_STYLE.clone())),
                None => ProgressBar::hidden(),
            },
            multi_progress: multi,
        }
    }
}
impl Reporter for ProgressReporter {
    type ItemRep<'a> = Item<'a>;
    fn start_item<O: Display, I: Display>(
        &self,
        operation: &str,
        prefix: &'static str,
        kind: &'static str,
        id: I,
        option: Option<O>,
    ) -> Self::ItemRep<'_> {
        self.progress_bar.inc(1);
        self.progress_bar
            .set_message(format!("processing {} {}", kind, id));
        Item::new(&self.multi_progress, operation, prefix, kind, id, option)
    }
    fn link_item<I: Display, P: AsRef<Path>>(&self, kind: &str, id: I, dest: P) {
        Item::link(&self.multi_progress, kind, id, dest)
    }

    type ItemContainerRep<'a> = Container<'a>;
    fn start_item_container<II, IO, IC, I, O>(
        &self,
        operation: &str,
        prefix: &'static str,
        id: I,
        option: Option<O>,
    ) -> Self::ItemContainerRep<'_>
    where
        II: item::Item,
        IC: item::ItemContainer<IO, II>,
        I: Display,
        O: Display,
    {
        self.progress_bar.inc(1);
        self.progress_bar
            .set_message(format!("processing {} in {} {}", II::TYPE, IC::TYPE, id));
        Container::new::<II, IO, IC, I, O>(&self.multi_progress, operation, prefix, id, option)
    }
    fn link_container<II, IO, IC, I, P>(&self, id: I, dest: P)
    where
        II: item::Item,
        IC: item::ItemContainer<IO, II>,
        I: Display,
        P: AsRef<Path>,
    {
        Container::link::<II, IO, IC, I, P>(&self.multi_progress, id, dest)
    }

    type JobRep<'a> = Job<'a> where Self:'a;
    fn start_job<I: Display>(&self, operation: &str, msg: I) -> Self::JobRep<'_> {
        Job::new(&self.multi_progress, operation, msg)
    }
}
