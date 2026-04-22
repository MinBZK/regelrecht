// Generate a synthetic Dutch-ish population for the Zorgtoeslag simulation.
// Numbers are deliberately simple — the goal is illustrative spread, not CBS accuracy.

function randomBsn(index) {
  // Deterministic per index so reruns produce identical populations.
  return String(900000000 + index).padStart(9, '0');
}

function randomAge(rng) {
  // Uniform 18..89
  return 18 + Math.floor(rng() * 72);
}

function ageToBirthdate(age, calculationDate) {
  const [y, m, d] = calculationDate.split('-').map((v) => parseInt(v, 10));
  return `${y - age}-${String(m).padStart(2, '0')}-${String(d).padStart(2, '0')}`;
}

function logNormal(rng, mu, sigma) {
  // Box-Muller for a standard normal, then exp to log-normal.
  const u1 = Math.max(rng(), 1e-10);
  const u2 = rng();
  const z = Math.sqrt(-2 * Math.log(u1)) * Math.cos(2 * Math.PI * u2);
  return Math.exp(mu + sigma * z);
}

// Seeded mulberry32 — fast, good-enough for demo statistics.
function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function generatePopulation({ size, seed = 42, calculationDate }) {
  const rng = mulberry32(seed);
  const out = new Array(size);
  for (let i = 0; i < size; i++) {
    const age = randomAge(rng);
    // median around €25k/year, heavy right tail.
    const inkomenEuros = Math.min(300000, Math.max(0, Math.round(logNormal(rng, 10.0, 0.7))));
    // vermogen mostly modest.
    const vermogenEuros = Math.min(500000, Math.max(0, Math.round(logNormal(rng, 7.5, 1.5))));
    const hasPartner = rng() < 0.45;
    out[i] = {
      bsn: randomBsn(i),
      geboortedatum: ageToBirthdate(age, calculationDate),
      age,
      inkomen: inkomenEuros * 100, // eurocent
      vermogen: vermogenEuros * 100,
      hasPartner,
      partner_bsn: hasPartner ? randomBsn(i + 1_000_000) : null,
    };
  }
  return out;
}

export function histogram(values, bins = 20) {
  if (!values.length) return { buckets: [], max: 0, min: 0, bins: 0 };
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = Math.max(1, max - min);
  const step = span / bins;
  const buckets = new Array(bins).fill(0).map((_, i) => ({
    from: min + i * step,
    to: min + (i + 1) * step,
    count: 0,
  }));
  for (const v of values) {
    const idx = Math.min(bins - 1, Math.floor((v - min) / step));
    buckets[idx].count += 1;
  }
  return { buckets, min, max, bins };
}
