use anndists::dist::DistCosine;
use hnsw_rs::hnsw::Hnsw;
use serde::{Deserialize, Serialize};

use crate::domain::embedding::{Embedding, VectorSearchError, VectorSearchResult, VectorSearcher};
use crate::domain::indexer::chunker::Chunk;
use crate::infra::vector_cache::CachedEmbeddings;

/// チャンクのメタデータ（ベクトルIDに紐づく）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMeta {
    pub chunk_id: usize,
    pub source_path: String,
    pub chunk_index: usize,
    pub text: String,
}

/// HNSWベクトルインデックス
pub struct HnswVectorIndex {
    hnsw: Hnsw<'static, f32, DistCosine>,
    metas: Vec<ChunkMeta>,
}

impl HnswVectorIndex {
    /// 新しいインデックスを作成する
    pub fn new() -> Self {
        let max_nb_connection = 16;
        let nb_elem = 50_000;
        let max_layer = 16;
        let ef_construction = 200;

        Self {
            hnsw: Hnsw::new(
                max_nb_connection,
                nb_elem,
                max_layer,
                ef_construction,
                DistCosine {},
            ),
            metas: Vec::new(),
        }
    }

    /// チャンクとそのembeddingを追加する
    pub fn add(&mut self, chunk: &Chunk, embedding: &Embedding) {
        let id = self.metas.len();
        self.metas.push(ChunkMeta {
            chunk_id: id,
            source_path: chunk.source_path.clone(),
            chunk_index: chunk.chunk_index,
            text: chunk.text.clone(),
        });
        self.hnsw.insert((embedding, id));
    }

    /// 複数のチャンクとembeddingを一括追加する
    pub fn add_batch(&mut self, chunks: &[Chunk], embeddings: &[Embedding]) {
        let start_id = self.metas.len();
        for (i, chunk) in chunks.iter().enumerate() {
            self.metas.push(ChunkMeta {
                chunk_id: start_id + i,
                source_path: chunk.source_path.clone(),
                chunk_index: chunk.chunk_index,
                text: chunk.text.clone(),
            });
        }

        let data: Vec<(&Vec<f32>, usize)> = embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| (emb, start_id + i))
            .collect();
        self.hnsw.parallel_insert(&data);
    }

    /// インデックスに登録されたチャンク数を返す
    pub fn len(&self) -> usize {
        self.metas.len()
    }

    /// インデックスが空かどうかを返す
    pub fn is_empty(&self) -> bool {
        self.metas.is_empty()
    }

    /// キャッシュされたembedding+メタデータからHNSWインデックスを再構築する
    pub fn from_cache(cached: CachedEmbeddings) -> Self {
        let mut index = Self::new();
        index.metas = cached.metas;

        let data: Vec<(&Vec<f32>, usize)> = cached
            .embeddings
            .iter()
            .enumerate()
            .map(|(i, emb)| (emb, i))
            .collect();
        index.hnsw.parallel_insert(&data);

        index
    }

    /// メタデータへの参照を返す（キャッシュ保存用）
    pub fn metas(&self) -> &[ChunkMeta] {
        &self.metas
    }
}

impl VectorSearcher for HnswVectorIndex {
    fn search_nearest(
        &self,
        query_embedding: &Embedding,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, VectorSearchError> {
        if self.is_empty() {
            return Err(VectorSearchError::IndexNotFound);
        }

        let ef_search = 200;
        let neighbours = self.hnsw.search(query_embedding, limit, ef_search);

        let results: Vec<VectorSearchResult> = neighbours
            .iter()
            .filter_map(|n| {
                let id = n.d_id;
                self.metas.get(id).map(|meta| VectorSearchResult {
                    chunk_id: meta.chunk_id,
                    source_path: meta.source_path.clone(),
                    distance: n.distance,
                    text: meta.text.clone(),
                })
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::embedding::VectorSearcher;
    use crate::domain::indexer::chunker::Chunk;

    fn random_embedding(dim: usize, seed: u64) -> Embedding {
        // 簡易的な疑似乱数（テスト用）
        let mut v = Vec::with_capacity(dim);
        let mut s = seed;
        for _ in 0..dim {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            v.push((s as f32) / (u64::MAX as f32));
        }
        // L2正規化
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        v.iter_mut().for_each(|x| *x /= norm);
        v
    }

    #[test]
    fn test_add_and_search() {
        let mut index = HnswVectorIndex::new();

        let chunk = Chunk {
            source_path: "/test.md".to_string(),
            chunk_index: 0,
            text: "テストチャンク".to_string(),
        };
        let embedding = random_embedding(384, 42);
        index.add(&chunk, &embedding);

        assert_eq!(index.len(), 1);

        // 同じベクトルで検索 → 自分自身が最近傍
        let results = index.search_nearest(&embedding, 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_path, "/test.md");
        assert_eq!(results[0].text, "テストチャンク");
    }

    #[test]
    fn test_batch_add_and_search() {
        let mut index = HnswVectorIndex::new();

        let chunks: Vec<Chunk> = (0..10)
            .map(|i| Chunk {
                source_path: format!("/doc{}.md", i),
                chunk_index: 0,
                text: format!("ドキュメント{}", i),
            })
            .collect();

        let embeddings: Vec<Embedding> = (0..10)
            .map(|i| random_embedding(384, i as u64 * 100 + 1))
            .collect();

        index.add_batch(&chunks, &embeddings);
        assert_eq!(index.len(), 10);

        // 特定のベクトルで検索
        let results = index.search_nearest(&embeddings[3], 3).unwrap();
        assert!(!results.is_empty());
        // 最近傍は自分自身のはず
        assert_eq!(results[0].source_path, "/doc3.md");
    }

    #[test]
    fn test_empty_index_returns_error() {
        let index = HnswVectorIndex::new();
        let query = random_embedding(384, 1);
        let result = index.search_nearest(&query, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_search_respects_limit() {
        let mut index = HnswVectorIndex::new();

        let chunks: Vec<Chunk> = (0..20)
            .map(|i| Chunk {
                source_path: format!("/doc{}.md", i),
                chunk_index: 0,
                text: format!("ドキュメント{}", i),
            })
            .collect();

        let embeddings: Vec<Embedding> = (0..20)
            .map(|i| random_embedding(384, i as u64 * 50 + 7))
            .collect();

        index.add_batch(&chunks, &embeddings);

        let results = index.search_nearest(&embeddings[0], 5).unwrap();
        assert!(results.len() <= 5);
    }

    #[test]
    fn test_from_cache_rebuild() {
        // 元のインデックスを構築
        let mut index = HnswVectorIndex::new();
        let chunks: Vec<Chunk> = (0..5)
            .map(|i| Chunk {
                source_path: format!("/doc{}.md", i),
                chunk_index: 0,
                text: format!("ドキュメント{}", i),
            })
            .collect();
        let embeddings: Vec<Embedding> = (0..5)
            .map(|i| random_embedding(384, i as u64 * 100 + 1))
            .collect();

        for (chunk, emb) in chunks.iter().zip(embeddings.iter()) {
            index.add(chunk, emb);
        }

        // キャッシュから再構築
        let cached = CachedEmbeddings {
            metas: index.metas().to_vec(),
            embeddings: embeddings.clone(),
        };
        let rebuilt = HnswVectorIndex::from_cache(cached);

        assert_eq!(rebuilt.len(), 5);
        let results = rebuilt.search_nearest(&embeddings[2], 1).unwrap();
        assert_eq!(results[0].source_path, "/doc2.md");
    }

    #[test]
    fn test_from_cache_with_deletion() {
        // 5件のチャンクからdoc2を除いた4件で再構築
        let embeddings: Vec<Embedding> = (0..5)
            .map(|i| random_embedding(384, i as u64 * 100 + 1))
            .collect();
        let metas: Vec<ChunkMeta> = (0..5)
            .map(|i| ChunkMeta {
                chunk_id: i,
                source_path: format!("/doc{}.md", i),
                chunk_index: 0,
                text: format!("ドキュメント{}", i),
            })
            .collect();

        // doc2を除外してキャッシュを構築
        let mut filtered_metas = Vec::new();
        let mut filtered_embeddings = Vec::new();
        for (meta, emb) in metas.into_iter().zip(embeddings.iter()) {
            if meta.source_path != "/doc2.md" {
                filtered_metas.push(meta);
                filtered_embeddings.push(emb.clone());
            }
        }
        // chunk_idを振り直す
        for (i, meta) in filtered_metas.iter_mut().enumerate() {
            meta.chunk_id = i;
        }

        let cached = CachedEmbeddings {
            metas: filtered_metas,
            embeddings: filtered_embeddings,
        };
        let index = HnswVectorIndex::from_cache(cached);

        assert_eq!(index.len(), 4);
        // doc2のembeddingで検索しても、doc2自体は結果に出ない
        let results = index.search_nearest(&embeddings[2], 4).unwrap();
        assert!(
            results.iter().all(|r| r.source_path != "/doc2.md"),
            "削除されたドキュメントは検索結果に含まれない"
        );
    }

    #[test]
    fn test_from_cache_with_addition() {
        // 3件のキャッシュに2件追加して5件で再構築
        let embeddings: Vec<Embedding> = (0..5)
            .map(|i| random_embedding(384, i as u64 * 100 + 1))
            .collect();
        let metas: Vec<ChunkMeta> = (0..5)
            .enumerate()
            .map(|(i, _)| ChunkMeta {
                chunk_id: i,
                source_path: format!("/doc{}.md", i),
                chunk_index: 0,
                text: format!("ドキュメント{}", i),
            })
            .collect();

        let cached = CachedEmbeddings {
            metas,
            embeddings: embeddings.clone(),
        };
        let index = HnswVectorIndex::from_cache(cached);

        assert_eq!(index.len(), 5);
        // 追加されたdoc3, doc4が検索可能
        let results = index.search_nearest(&embeddings[4], 1).unwrap();
        assert_eq!(results[0].source_path, "/doc4.md");
    }
}
