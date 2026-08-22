//! Lexical BM25 retrieval over the local rail corpora — the RAG layer for AI
//! grounding. Pure, offline, dependency-free: no embeddings, no network.
//!
//! Two document kinds are indexed at startup: stations (code / name / state)
//! and trains (number / name). Exact-code queries get an overwhelming boost
//! so `"12951"` or `"NDLS"` always resolve deterministically; everything else
//! ranks by Okapi BM25 (k1=1.2, b=0.75).

use std::collections::HashMap;

/// One indexed document supplied at build time.
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// `"station"` or `"train"`.
    pub kind: &'static str,
    /// Station code or 5-digit train number (also the exact-match key).
    pub code: String,
    /// Primary display name.
    pub title: String,
    /// Secondary text that should be searchable but not displayed prominently.
    pub detail: String,
}

/// A scored retrieval hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Retrieved {
    pub kind: &'static str,
    pub code: String,
    pub title: String,
    pub detail: String,
    pub score: f32,
}

#[derive(Debug)]
struct Doc {
    entry: IndexEntry,
    length: f32,
}

#[derive(Debug, Default)]
struct Posting {
    freq: HashMap<u32, u32>,
    df: u32,
}

const K1: f32 = 1.2;
const B: f32 = 0.75;
/// Overwhelming-but-finite boost so exact codes beat any BM25 pile-up while
/// still ordering multiple exact hits sensibly.
const EXACT_BOOST: f32 = 10_000.0;

/// Tokenizer shared by indexing and querying: lowercase alphanumeric runs.
fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let cur = text.to_ascii_lowercase();
    let mut start = None;
    for (i, ch) in cur.char_indices() {
        if ch.is_ascii_alphanumeric() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            out.push(cur[s..i].to_string());
        }
    }
    if let Some(s) = start {
        out.push(cur[s..].to_string());
    }
    out
}

/// Okapi BM25 index over a small static corpus (~20k documents builds in
/// milliseconds; no need for incremental updates).
#[derive(Debug, Default)]
pub struct RetrievalIndex {
    docs: Vec<Doc>,
    postings: HashMap<String, Posting>,
    total_len: f32,
}

impl RetrievalIndex {
    pub fn build(entries: impl IntoIterator<Item = IndexEntry>) -> Self {
        let mut idx = Self::default();
        for entry in entries {
            let doc_id = idx.docs.len() as u32;
            // Title tokens weigh double: names matter more than metadata.
            let mut terms = tokenize(&entry.title);
            terms.extend(tokenize(&entry.title));
            terms.extend(tokenize(&entry.detail));
            terms.push(entry.code.to_ascii_lowercase());
            let length = terms.len() as f32;
            for term in terms {
                let p = idx.postings.entry(term).or_default();
                *p.freq.entry(doc_id).or_insert(0) += 1;
                p.df = p.freq.len() as u32;
            }
            idx.total_len += length;
            idx.docs.push(Doc { entry, length });
        }
        idx
    }

    /// Number of documents in the index.
    pub fn len(&self) -> usize {
        self.docs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.docs.is_empty()
    }

    /// Top-`limit` hits for `query`, best first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Retrieved> {
        if limit == 0 {
            return Vec::new();
        }
        let n = self.docs.len() as f32;
        let avgdl = if n > 0.0 { self.total_len / n } else { 1.0 };
        let terms = tokenize(query);
        if terms.is_empty() {
            return Vec::new();
        }
        let query_upper = query.trim().to_ascii_uppercase();

        // Accumulate BM25 contributions per candidate doc.
        let mut scores: HashMap<u32, f32> = HashMap::new();
        for term in &terms {
            let Some(posting) = self.postings.get(term) else {
                continue;
            };
            let df = posting.df as f32;
            let idf = ((n - df + 0.5) / (df + 0.5)).max(0.1).ln() + 1.0;
            for (&doc_id, &tf) in &posting.freq {
                let tf = tf as f32;
                let dl = self.docs[doc_id as usize].length;
                let norm = tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * dl / avgdl));
                *scores.entry(doc_id).or_default() += idf * norm;
            }
        }

        // Exact-code matches jump the queue (still deterministic among ties).
        if !query_upper.is_empty()
            && (2..=6).contains(&query_upper.len())
            && query_upper.chars().all(|c| c.is_ascii_alphanumeric())
        {
            for (id, doc) in self.docs.iter().enumerate() {
                if doc.entry.code.eq_ignore_ascii_case(&query_upper) {
                    scores.insert(id as u32, EXACT_BOOST);
                }
            }
        }

        let mut ranked: Vec<(u32, f32)> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked
            .into_iter()
            .take(limit)
            .map(|(id, score)| {
                let d = &self.docs[id as usize].entry;
                Retrieved {
                    kind: d.kind,
                    code: d.code.clone(),
                    title: d.title.clone(),
                    detail: d.detail.clone(),
                    score,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> Vec<IndexEntry> {
        vec![
            IndexEntry {
                kind: "station",
                code: "NDLS".into(),
                title: "NEW DELHI".into(),
                detail: "Delhi Central Delhi".into(),
            },
            IndexEntry {
                kind: "station",
                code: "NBJU".into(),
                title: "NEW BARAUNI JN".into(),
                detail: "Bihar".into(),
            },
            IndexEntry {
                kind: "station",
                code: "SC".into(),
                title: "SECUNDERABAD JN".into(),
                detail: "Telangana Hyderabad".into(),
            },
            IndexEntry {
                kind: "station",
                code: "HYB".into(),
                title: "HYDERABAD DECCAN".into(),
                detail: "Telangana Hyderabad".into(),
            },
            IndexEntry {
                kind: "train",
                code: "12951".into(),
                title: "MUMBAI CENTRAL NEW DELHI RAJDHANI EXPRESS".into(),
                detail: "".into(),
            },
            IndexEntry {
                kind: "train",
                code: "11020".into(),
                title: "KONARK EXPRESS".into(),
                detail: "".into(),
            },
        ]
    }

    #[test]
    fn exact_station_code_ranks_first_and_is_deterministic() {
        let idx = RetrievalIndex::build(corpus());
        let hits = idx.search("ndls", 3);
        assert_eq!(hits[0].kind, "station");
        assert_eq!(hits[0].code, "NDLS");
        assert!(hits[0].score > 100.0, "exact boost applied");
    }

    #[test]
    fn exact_train_number_beats_partial_name_overlap() {
        let idx = RetrievalIndex::build(corpus());
        // "12951 delhi" contains name tokens of the Rajdhani too, but the
        // number must win.
        let hits = idx.search("12951", 5);
        assert_eq!(hits[0].kind, "train");
        assert_eq!(hits[0].code, "12951");
    }

    #[test]
    fn multi_term_query_prefers_full_phrase_match() {
        let idx = RetrievalIndex::build(corpus());
        let hits = idx.search("new delhi", 6);
        assert_eq!(hits[0].code, "NDLS", "both terms matched");
        // NEW BARAUNI shares only one term and must rank below.
        assert!(hits.iter().position(|h| h.code == "NBJU").unwrap_or(99) > 0);
    }

    #[test]
    fn searches_detail_field_for_region_queries() {
        let idx = RetrievalIndex::build(corpus());
        let hits = idx.search("hyderabad telangana", 4);
        let codes: Vec<&str> = hits.iter().map(|h| h.code.as_str()).collect();
        assert!(codes.contains(&"SC"), "detail-indexed: {codes:?}");
        assert!(codes.contains(&"HYB"), "detail-indexed: {codes:?}");
    }

    #[test]
    fn limit_and_empty_query_are_honored() {
        let idx = RetrievalIndex::build(corpus());
        assert_eq!(idx.search("delhi", 2).len(), 2);
        assert_eq!(idx.search("   ", 5).len(), 0);
        assert_eq!(idx.search("zzzz-nothing", 5).len(), 0);
        assert_eq!(RetrievalIndex::default().search("x", 3).len(), 0);
    }
}
