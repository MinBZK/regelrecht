// Elke URL die deze app zelf ophaalt, relatief aan waar de app gemonteerd is.
//
// Los draait napp op `/`, achter het poc-portaal op `/napp/`. Vite zet
// `import.meta.env.BASE_URL` op wat er in `base` staat (met een afsluitende
// slash), dus dat is de enige plek waar het pad vandaan komt.
//
// Anders dan bij de statische pocs praat deze app met een eigen backend. Die
// backend serveert zichzelf óók onder het voorvoegsel (NAPP_BASE_PATH), zodat
// napp los blijft werken en het portaal het pad ongewijzigd kan doorsturen.
export const b = (pad) => import.meta.env.BASE_URL + String(pad).replace(/^\//, '');
