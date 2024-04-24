// NOTE: Courtesy Alireza on SO
// https://stackoverflow.com/a/57949518
const isLocal = _ => Boolean(
    window.location.hostname === 'localhost' || //
    window.location.hostname === '[::1]' || //
    window.location.hostname.match(
        /^127(?:\.(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)){3}$/
    )
);


import("../pkg/index.js").then(module => {
    let sinceLastResize;

    window.onresize = _ => {
        clearTimeout(sinceLastResize);

        sinceLastResize = setTimeout(_ => {
            module.update_viewport(`{
                "width": ${window.innerWidth},
                "height": ${window.innerHeight}
            }`);
        }, 500);
    };

    module.run_wasm();
}).catch(console.error);