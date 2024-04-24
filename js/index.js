// NOTE: Courtesy Alireza on SO
// https://stackoverflow.com/a/57949518
const isLocal = _ => Boolean(
    window.location.hostname === 'localhost' || //
    window.location.hostname === '[::1]' || //
    window.location.hostname.match(
        /^127(?:\.(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)){3}$/
    )
);

import("../pkg/index.js").then(module => module.run()).catch(console.error);