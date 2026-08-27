#!/usr/bin/env python3
"""Cross-encoder reranker untuk LongMemEval harness (issue #1118).

Pipeline: retrieve top-N (hybrid) -> score (query, session_text) dengan
cross-encoder ONNX (ms-marco-MiniLM-L-6-v2) -> reorder -> top-k.

Model: /opt/data/.codecora/uteke/models/ms-marco-minilm-l6/model.onnx
Deterministik: tidak ada sampling; input sama -> urutan sama.
"""
import json
import numpy as np
import onnxruntime as ort
from pathlib import Path
from tokenizers import Tokenizer

MODEL_DIR = Path("/opt/data/.codecora/uteke/models/ms-marco-minilm-l6")


class CrossEncoderReranker:
    def __init__(self, model_path: str = None, max_len: int = 256):
        mp = Path(model_path) if model_path else MODEL_DIR / "model.onnx"
        self.tok = Tokenizer.from_file(str(MODEL_DIR / "tokenizer.json"))
        # pasangan (query, passage) -> token type id 1 (standard cross-encoder)
        self.tok.encode_special_tokens = True
        so = ort.SessionOptions()
        so.intra_op_num_threads = 2
        self.sess = ort.InferenceSession(str(mp), so, providers=["CPUExecutionProvider"])
        self.max_len = max_len

    def _encode_pair(self, query: str, passage: str):
        # ms-marco MiniLM tokenizer: text pair via encode
        enc = self.tok.encode(query, passage)
        ids = enc.ids[: self.max_len]
        mask = enc.attention_mask[: self.max_len]
        # pad ke max_len agar batch-able
        pad = self.max_len - len(ids)
        ids = ids + [0] * pad
        mask = mask + [0] * pad
        return ids, mask

    def score(self, query: str, passages: list) -> np.ndarray:
        """Score list of passages against query. Returns array of scores."""
        ids_all, mask_all = [], []
        for p in passages:
            i, m = self._encode_pair(query, p)
            ids_all.append(i)
            mask_all.append(m)
        feeds = {
            "input_ids": np.array(ids_all, dtype=np.int64),
            "attention_mask": np.array(mask_all, dtype=np.int64),
        }
        # model bisa punya token_type_ids input opsional
        inp_names = [i.name for i in self.sess.get_inputs()]
        if "token_type_ids" in inp_names:
            feeds["token_type_ids"] = np.zeros_like(feeds["input_ids"])
        out = self.sess.run(None, feeds)[0]
        # output shape (batch, 1) logits
        return out.reshape(-1)

    def rerank(self, query: str, items: list, text_key: str = "text", k: int = 5) -> list:
        """items: list of dict with 'text' (session text). Returns top-k items reordered."""
        if not items:
            return []
        passages = [it.get(text_key, "")[:4000] for it in items]
        scores = self.score(query, passages)
        order = np.argsort(-scores)  # descending, stabil untuk tie by index
        return [items[i] for i in order[:k]]


if __name__ == "__main__":
    import sys
    rr = CrossEncoderReranker()
    q = "What did the user eat last month?"
    docs = [
        "User discussed pizza preferences and ordered pepperoni.",
        "The weather in Jakarta is hot today.",
        "User mentioned eating sushi with friends in June.",
        "Python programming language discussion.",
        "User's favorite movie is Interstellar.",
    ]
    s = rr.score(q, docs)
    for i in np.argsort(-s):
        print(f"{s[i]:+.4f}  {docs[i][:60]}")
