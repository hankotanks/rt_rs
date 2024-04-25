const package = require('../pkg/package.json');

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
    module.run_wasm();

    const resizeCanvas = _ => {
        module.update_viewport(`{
            "width": ${window.innerWidth},
            "height": ${window.innerHeight}
        }`);
    };

    resizeCanvas();

    let sinceLastResize;
    window.onresize = _ => {
        clearTimeout(sinceLastResize);

        sinceLastResize = setTimeout(resizeCanvas, 300);
    };

    const loadScene = sceneName => {
        let root = window.location.origin;
        if(!isLocal()) { root += `/${package.name}`; }

        fetch(`${root}/scenes/${sceneName}.json`).then(response => {
            if (!response.ok) { throw new Error(`Failed to retrieve scene [${sceneName}]`); }

            return response.text();
        }).then(sceneSerial => {
            module.update_scene(sceneSerial);
        }).catch(console.error);
    };

    document.getElementById("config-load-default").onclick = _ => {
        loadScene('default');
    };

    
}).catch(console.error);