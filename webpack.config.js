const path = require('path');

const TerserPlugin = require('terser-webpack-plugin');
const CopyPlugin = require('copy-webpack-plugin');
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');

const dist = path.resolve(__dirname, 'dist');

module.exports = {
    mode: 'production',
    entry: {
        index: './js/index.js'
    },
    output: {
        path: dist,
        filename: '[name].js',
        publicPath: process.env.DEPLOY_PAGES ? '/rt_rs/' : undefined
    },
    devServer: {
        contentBase: dist,
    },
    plugins: [
        new CopyPlugin([
            path.resolve(__dirname, 'static')
        ]),
        new CopyPlugin([
            { from: path.resolve(__dirname, "scenes"), to: "scenes" }
          ]),
        new WasmPackPlugin({
            crateDirectory: __dirname,
        }),
    ],
    optimization: {
        minimize: true,
        minimizer: [
            new TerserPlugin({
                terserOptions: {
                    ecma: undefined,
                    parse: {},
                    compress: {},
                    mangle: true,
                    module: true,
                },
            }),
        ],
    }
};
