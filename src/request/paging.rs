use super::{Client, Signer};
use crate::{progress, raw_data::RawData};
use chrono::{DateTime, Utc};
use reqwest::{IntoUrl, Method};
use serde::{de, Deserialize};
use std::collections::LinkedList;

#[derive(Deserialize)]
struct Paging {
    is_end: bool,
    #[serde(default)]
    totals: Option<u64>,
    next: String,
}

fn deserialize_data<'de, D: de::Deserializer<'de>>(
    deserializer: D,
) -> Result<LinkedList<RawData>, D::Error> {
    struct DataVisitor(DateTime<Utc>);
    impl<'de> de::Visitor<'de> for DataVisitor {
        type Value = LinkedList<RawData>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("data")
        }
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut ret = LinkedList::new();
            while let Some(v) = seq.next_element()? {
                ret.push_back(RawData {
                    fetch_time: self.0,
                    data: v,
                });
            }
            Ok(ret)
        }
    }
    deserializer.deserialize_seq(DataVisitor(Utc::now()))
}

#[derive(Deserialize)]
struct PagedData {
    #[serde(default, deserialize_with = "deserialize_data")]
    data: LinkedList<RawData>,
    #[serde(default)]
    paging: Option<Paging>,
}

impl Client {
    pub(crate) async fn get_paged_sign<S: Signer, P: progress::FetchProg, U: IntoUrl>(
        &self,
        mut prog: P,
        url: U,
    ) -> reqwest::Result<LinkedList<RawData>> {
        let (mut ret, mut paging) = {
            let pd = self
                .request_signed::<S, U>(Method::GET, url)
                .send()
                .await?
                .json::<PagedData>()
                .await?;
            (pd.data, pd.paging)
        };
        prog.set_count(match &paging {
            Some(p) => p.totals,
            None => None,
        });
        prog.inc(ret.len() as u64);
        prog.sleep(self.request_interval).await;
        while let Some(Paging {
            is_end: false,
            next,
            ..
        }) = paging
        {
            let mut pd = self
                .request_signed::<S, String>(Method::GET, next)
                .send()
                .await?
                .json::<PagedData>()
                .await?;
            prog.inc(pd.data.len() as u64);
            ret.append(&mut pd.data);
            paging = pd.paging;
            prog.sleep(self.request_interval).await;
        }
        Ok(ret)
    }
    pub(crate) async fn get_paged<P: progress::FetchProg, U: IntoUrl>(
        &self,
        prog: P,
        url: U,
    ) -> reqwest::Result<LinkedList<RawData>> {
        self.get_paged_sign::<super::NoSign, P, U>(prog, url).await
    }
}
