import { Effect, Schema } from "effect";
import { Vibrant } from "node-vibrant/node";
import { inter, notoSans } from "./fonts";
import satori from "satori";
import { OpenGraph } from "@spqtify/embed-image";
import { Resvg } from "@resvg/resvg-js";

export class GetAlbumArtError extends Schema.TaggedError<GetAlbumArtError>()(
    "GetAlbumArtError",
    {},
) {}

// TODO: return placeholder image on errors instead?
// stops request from failing if spotify cdn is down
export const getArtworkArrayBuffer = (albumArtUrl: string) =>
    Effect.gen(function* () {
        const imageArrayBuffer = yield* Effect.tryPromise({
            try: () => fetch(albumArtUrl).then((res) => res.arrayBuffer()),
            // TODO: how can i not eat this error
            catch: () => new GetAlbumArtError(),
        });

        return imageArrayBuffer;
    });

export class GetImagePaletteError extends Schema.TaggedError<GetImagePaletteError>()(
    "GetImagePaletteError",
    {},
) {}

export const getPaletteFromImage = (imageArrayBuffer: ArrayBuffer) =>
    Effect.gen(function* () {
        const palette = yield* Effect.tryPromise({
            try: () => Vibrant.from(Buffer.from(imageArrayBuffer)).getPalette(),
            // TODO: how can i not eat this error
            catch: () => new GetImagePaletteError(),
        });

        return palette;
    });

export class GenerateSvgDataError extends Schema.TaggedError<GenerateSvgDataError>()(
    "GenerateSvgDataError",
    {},
) {}

export class GeneratePngError extends Schema.TaggedError<GeneratePngError>()(
    "GeneratePngError",
    {},
) {}

export const generateImage = (opts: {
    albumArt: ArrayBuffer;
    baseColor: string;
    gradientColor: string;
    songName: string;
    artist: string;
}) =>
    Effect.gen(function* () {
        const svgComp = OpenGraph(opts);

        const svgData = yield* Effect.tryPromise({
            try: () =>
                satori(svgComp, {
                    width: 800,
                    height: 300,
                    fonts: [
                        {
                            name: "Inter",
                            data: inter[400],
                            weight: 400,
                        },
                        {
                            name: "Inter",
                            data: inter[700],
                            weight: 700,
                        },
                        {
                            name: "Noto Sans JP",
                            data: notoSans[400],
                            weight: 400,
                        },
                        {
                            name: "Noto Sans JP",
                            data: notoSans[700],
                            weight: 700,
                        },
                    ],
                }),
            catch: () => new GenerateSvgDataError(),
        });

        const pngBuffer = yield* Effect.try({
            try: () => {
                const resvg = new Resvg(svgData, {});
                return resvg.render().asPng();
            },
            catch: () => new GeneratePngError(),
        });

        return pngBuffer;
    });
