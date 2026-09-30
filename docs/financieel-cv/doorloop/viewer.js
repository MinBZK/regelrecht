
(function () {
  var DATA = window.__TRACES__ || {};
  var MERK = {
    article: "\u25a4", cross_law_reference: "\u21d7", open_term_resolution: "\u21e9",
    hook_resolution: "\u2691", action: "\u25b8", operation: "\u2295",
    resolve: "\u25c6", cached: "\u27f3", requirement: "\u2713"
  };
  var BRON = {
    PARAMETER: "aangeleverd", OUTPUT: "afgeleid", DEFINITION: "definitie", CONTEXT: "context",
    RESOLVED_INPUT: "uit andere wet", DATA_SOURCE: "bron", OPEN_TERM: "open norm",
    LOCAL: "tussenstap", INPUT: "invoer", URI: "verwijzing"
  };

  function waarde(v) {
    if (v === null || v === undefined) return null;
    if (typeof v === "boolean") return { t: v ? "v-true" : "v-false", s: v ? "waar" : "onwaar" };
    if (typeof v === "number") return { t: "v-num", s: String(v) };
    if (typeof v === "string") return { t: "v-str", s: "'" + v + "'" };
    if (Array.isArray(v)) return { t: "v-str", s: "[" + v.length + "]" };
    if (typeof v === "object") {
      var k = Object.keys(v);
      if (k.length === 1) return waarde(v[k[0]]);
    }
    return { t: "v-str", s: JSON.stringify(v) };
  }

  function tel(n) { var c = 1; (n.children || []).forEach(function (k) { c += tel(k); }); return c; }

  function bouw(node, diepte) {
    var wrap = document.createElement("div");
    wrap.className = "tk";
    var kinderen = node.children || [];
    var groot = tel(node) > 12;
    if (diepte >= 2 && groot) wrap.classList.add("dicht");

    var rij = document.createElement("div");
    rij.className = "tk-rij";
    if (node.node_type === "cross_law_reference") rij.classList.add("is-xlaw");
    if (node.node_type === "open_term_resolution") rij.classList.add("is-open-term");
    rij.style.paddingLeft = (0.7 + diepte * 1.15) + "rem";

    var knop = document.createElement("button");
    knop.type = "button";
    if (kinderen.length) {
      knop.className = "tk-knop";
      knop.textContent = wrap.classList.contains("dicht") ? "+" : "\u2212";
      knop.setAttribute("aria-label", "in- of uitklappen");
      knop.addEventListener("click", function () {
        wrap.classList.toggle("dicht");
        knop.textContent = wrap.classList.contains("dicht") ? "+" : "\u2212";
      });
    } else {
      knop.className = "tk-knop leeg";
      knop.textContent = "\u00b7";
      knop.tabIndex = -1;
      knop.setAttribute("aria-hidden", "true");
    }
    rij.appendChild(knop);

    var merk = document.createElement("span");
    merk.className = "tk-merk n-" + node.node_type;
    merk.textContent = MERK[node.node_type] || "\u00b7";
    merk.title = ({article:"artikel",cross_law_reference:"verwijzing naar andere wet",open_term_resolution:"open norm",hook_resolution:"Awb-bepaling",action:"rechtsregel",operation:"berekening",resolve:"gegeven opgehaald",cached:"eerder bepaald",requirement:"voorwaarde"})[node.node_type] || node.node_type;
    rij.appendChild(merk);

    var naam = document.createElement("span");
    naam.className = "tk-naam n-" + node.node_type;
    naam.textContent = node.name;
    rij.appendChild(naam);

    if (node.resolve_type) {
      var b = document.createElement("span");
      b.className = "tk-bron b-" + node.resolve_type;
      b.textContent = BRON[node.resolve_type] || node.resolve_type.toLowerCase();
      rij.appendChild(b);
    }

    var w = waarde(node.result);
    if (w) {
      var wv = document.createElement("span");
      wv.className = "tk-waarde " + w.t;
      wv.textContent = "= " + w.s;
      rij.appendChild(wv);
    }

    if (node.message && node.node_type !== "resolve") {
      var m = document.createElement("span");
      m.className = "tk-msg";
      m.textContent = node.message.split("\n")[0];
      rij.appendChild(m);
    }

    wrap.appendChild(rij);
    kinderen.forEach(function (k) {
      var kind = bouw(k, diepte + 1);
      kind.classList.add("tk-kind");
      wrap.appendChild(kind);
    });
    return wrap;
  }

  function render(el) {
    var d = DATA[el.getAttribute("data-trace")];
    if (!d) { el.textContent = "afleiding niet gevonden"; return; }

    var bar = document.createElement("div");
    bar.className = "boom-bar";

    function knopje(label, fn, toggle) {
      var b = document.createElement("button");
      b.type = "button";
      b.textContent = label;
      if (toggle) b.setAttribute("aria-pressed", "false");
      b.addEventListener("click", function () { fn(b); });
      bar.appendChild(b);
      return b;
    }

    var body = document.createElement("div");
    body.className = "boom-body";
    body.appendChild(bouw(d.trace, 0));

    function alle(dicht) {
      body.querySelectorAll(".tk").forEach(function (t) {
        if (!t.querySelector(":scope > .tk-kind")) return;
        t.classList.toggle("dicht", dicht);
        var k = t.querySelector(":scope > .tk-rij > .tk-knop");
        if (k && !k.classList.contains("leeg")) k.textContent = dicht ? "+" : "\u2212";
      });
    }

    knopje("alles uitklappen", function () { alle(false); });
    knopje("alles inklappen", function () { alle(true); });
    knopje("alleen verwijzingen naar andere wetten", function (b) {
      var aan = b.getAttribute("aria-pressed") !== "true";
      b.setAttribute("aria-pressed", aan ? "true" : "false");
      el.classList.toggle("only-x", aan);
      if (aan) alle(false);
    }, true);

    var t = document.createElement("span");
    t.className = "telling";
    t.textContent = tel(d.trace) + " redeneerstappen";
    bar.appendChild(t);

    el.appendChild(bar);
    el.appendChild(body);
  }

  function init() {
    document.querySelectorAll(".boom").forEach(function (el) {
      if (el.dataset.klaar) return;
      el.dataset.klaar = "1";
      render(el);
    });
  }

  // Bomen zitten in uitklapblokken; pas renderen als er een opengaat scheelt
  // bij het laden, maar houdt de pagina wel volledig doorzoekbaar zodra je
  // hem opent.
  document.addEventListener("toggle", function (e) {
    if (e.target.tagName === "DETAILS" && e.target.open) init();
  }, true);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {});
  }
})();
