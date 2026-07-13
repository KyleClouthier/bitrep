window.BENCHMARK_DATA = {
  "lastUpdate": 1783947729021,
  "repoUrl": "https://github.com/KyleClouthier/bitrep",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "4207a1256c6e0d7e92af1ac489ddde25dc77b4ec",
          "message": "Harden OpenSSF Scorecard + org copyright headers\n\n- SECURITY.md, least-privilege workflow permissions, SHA-pinned actions, CodeQL (SAST)\n- codecov.yml threshold so the coverage badge reads green honestly (~80%)\n- documented waiver for two unreachable PyO3 advisories (upgrade to 0.24+ tracked)\n- Clouthier Simulation Labs added to all source-file copyright headers (matches Cairn convention; MIT/Apache license terms unchanged)",
          "timestamp": "2026-07-13T08:19:29-04:00",
          "tree_id": "7bef901d330fd7a4cf25722b6759d62dd0366d09",
          "url": "https://github.com/KyleClouthier/bitrep/commit/4207a1256c6e0d7e92af1ac489ddde25dc77b4ec"
        },
        "date": 1783945406905,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3678,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2383,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2764,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2429,
            "range": "± 29",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93712,
            "range": "± 2756",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374450,
            "range": "± 458",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 275471,
            "range": "± 2869",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 492616,
            "range": "± 1005",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 510890,
            "range": "± 4780",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 938626,
            "range": "± 1205",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3749147,
            "range": "± 69643",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2692586,
            "range": "± 21862",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4957864,
            "range": "± 28882",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5068590,
            "range": "± 23827",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2650,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "66ea3a85a4bd375bbd58c816d8bfc2e07eecc70a",
          "message": "docs: state accumulator capacity as an explicit named limit (2^63 additions)",
          "timestamp": "2026-07-13T08:23:02-04:00",
          "tree_id": "ee3d93fbad031efc69684e256fc7a9e1f9b21355",
          "url": "https://github.com/KyleClouthier/bitrep/commit/66ea3a85a4bd375bbd58c816d8bfc2e07eecc70a"
        },
        "date": 1783945597393,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 771,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3224,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 1979,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2378,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2140,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 81808,
            "range": "± 47",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 327260,
            "range": "± 488",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 171388,
            "range": "± 275",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 416116,
            "range": "± 1073",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 437458,
            "range": "± 575",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 819665,
            "range": "± 615",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3276463,
            "range": "± 1448",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 1705677,
            "range": "± 5935",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4255834,
            "range": "± 7342",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 4400248,
            "range": "± 5753",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2312,
            "range": "± 3",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "b489d3800d9226ee2f3a5998d57ea5c37e92fa40",
          "message": "Simplify FloatGCounter proofs: drop redundant ejoin wrappers\n\nPer Lean Zulip feedback (Yan Yablonovskiy): the ejoin_comm/assoc/idem\nwrapper theorems were redundant restatements of core Nat.max lemmas.\njoin_comm/assoc/idem now defer directly to Nat.max_comm/assoc/self. Fewer\nhoops for a reviewer, same proof, still Lean-core-only with zero sorry.\nComparator theorem list unchanged (only ever referenced the join_* CRDT laws).",
          "timestamp": "2026-07-13T08:49:43-04:00",
          "tree_id": "3474d9cb03b98a9b2e0234120655aa44a39d7a7d",
          "url": "https://github.com/KyleClouthier/bitrep/commit/b489d3800d9226ee2f3a5998d57ea5c37e92fa40"
        },
        "date": 1783947205162,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3679,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2425,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2878,
            "range": "± 46",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2404,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93711,
            "range": "± 97",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374348,
            "range": "± 317",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 271628,
            "range": "± 2040",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 490938,
            "range": "± 4053",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 510750,
            "range": "± 1055",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 938350,
            "range": "± 933",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3744762,
            "range": "± 53372",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2737134,
            "range": "± 47347",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4956668,
            "range": "± 25071",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5063404,
            "range": "± 8912",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2650,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "f3a968a4fdc2e924d15409a37361cc1dd68b1aba",
          "message": "docs: cite the Lean manual's Validating a Lean Proof page for the comparator layer",
          "timestamp": "2026-07-13T08:58:28-04:00",
          "tree_id": "ada13bb64e8ffc25833bee329eeeb65b7ea29936",
          "url": "https://github.com/KyleClouthier/bitrep/commit/f3a968a4fdc2e924d15409a37361cc1dd68b1aba"
        },
        "date": 1783947728754,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3677,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2381,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2820,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2433,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93680,
            "range": "± 1315",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374214,
            "range": "± 659",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 269347,
            "range": "± 771",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 491477,
            "range": "± 11836",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 509662,
            "range": "± 1207",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 937096,
            "range": "± 1115",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3744116,
            "range": "± 3915",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2679834,
            "range": "± 14938",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4955000,
            "range": "± 7195",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5052206,
            "range": "± 124288",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2648,
            "range": "± 8",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}