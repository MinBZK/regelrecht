// Elke URL die deze app zelf ophaalt, relatief aan waar de app gemonteerd is.
//
// Lokaal draait hij op `/`, achter het poc-portaal op `/terugbetaalregimes/`.
// Vite zet `import.meta.env.BASE_URL` op wat er in `base` staat (met een
// afsluitende slash), dus dat is de enige plek waar het pad vandaan komt.
//
// `base` alleen is niet genoeg: dat herschrijft wat de bundler zelf uitgeeft
// (script- en link-tags, geïmporteerde assets), maar niet een string die wij in
// een `fetch()` zetten. Die staan hier.
export const b = (pad) => import.meta.env.BASE_URL + String(pad).replace(/^\//, '');
