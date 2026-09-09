#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use codspeed_criterion_compat::*;
use nom::{bytes::complete::take_until, error::Error, IResult};

fn until<'a>(needle: &'a [u8], input: &'a [u8]) -> IResult<&'a [u8], &'a [u8]> {
  take_until::<_, _, Error<&'a [u8]>>(needle)(input)
}

/// `filler` repeated until `len` bytes, then `needle` appended.
fn haystack(filler: &[u8], len: usize, needle: &[u8]) -> Vec<u8> {
  let mut v: Vec<u8> = filler.iter().copied().cycle().take(len).collect();
  v.extend_from_slice(needle);
  v
}

static HTTP: &[u8] = b"GET /index.html HTTP/1.1\r\n\
Host: www.example.com\r\n\
User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10.8; rv:15.0)\r\n\
Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n\
Accept-Language: en-us,en;q=0.5\r\n\
Accept-Encoding: gzip, deflate\r\n\
Connection: keep-alive\r\n\
\r\nBODY";

const ALPHA: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

fn cases() -> Vec<(&'static str, Vec<u8>, &'static [u8])> {
  vec![
    // Needle's first byte never appears in the filler, so the old
    // memchr-then-verify scan never does a wasted verification.
    (
      "rare_first_byte/16",
      haystack(ALPHA, 16, b"QUUX"),
      &b"QUUX"[..],
    ),
    (
      "rare_first_byte/64",
      haystack(ALPHA, 64, b"QUUX"),
      &b"QUUX"[..],
    ),
    (
      "rare_first_byte/1k",
      haystack(ALPHA, 1024, b"QUUX"),
      &b"QUUX"[..],
    ),
    (
      "rare_first_byte/16k",
      haystack(ALPHA, 16384, b"QUUX"),
      &b"QUUX"[..],
    ),
    // Needle's first byte is every byte of the filler, so the old scan
    // verifies at every offset while memmem prefilters on a rarer byte.
    (
      "common_first_byte/1k",
      haystack(b"a", 1024, b"aaab"),
      &b"aaab"[..],
    ),
    (
      "common_first_byte/16k",
      haystack(b"a", 16384, b"aaab"),
      &b"aaab"[..],
    ),
    // Single-byte needle: the old code delegates straight to memchr.
    ("needle_1b/16k", haystack(ALPHA, 16384, b"Q"), &b"Q"[..]),
    // Longer needle, and a full scan that finds nothing.
    (
      "needle_16b/16k",
      haystack(ALPHA, 16384, b"QUUXQUUXQUUXQUUX"),
      &b"QUUXQUUXQUUXQUUX"[..],
    ),
    ("not_found/16k", haystack(ALPHA, 16384, b""), &b"QUUX"[..]),
    // Realistic: locate the header/body separator.
    ("http_headers", HTTP.to_vec(), &b"\r\n\r\n"[..]),
  ]
}

fn bench_take_until(c: &mut Criterion) {
  let mut group = c.benchmark_group("take_until");
  for (name, hay, needle) in cases() {
    // Check outside the timing loop that each case does what it claims.
    let found = until(needle, &hay).is_ok();
    assert_eq!(
      found,
      name != "not_found/16k",
      "case {} misconfigured",
      name
    );

    group.throughput(Throughput::Bytes(hay.len() as u64));
    group.bench_function(BenchmarkId::from_parameter(name), |b| {
      b.iter(|| until(black_box(needle), black_box(hay.as_slice())))
    });
  }
  group.finish();
}

criterion_group!(benches, bench_take_until);
criterion_main!(benches);
