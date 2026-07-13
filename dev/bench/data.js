window.BENCHMARK_DATA = {
  "lastUpdate": 1783945407219,
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
      }
    ]
  }
}