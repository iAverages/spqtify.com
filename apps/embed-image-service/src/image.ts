import { Effect, Schema } from "effect";
import { Vibrant } from "node-vibrant/node";
import { inter, notoSans } from "./fonts";
import satori from "satori";
import {
    OpenGraph,
    OpenGraphAlbum,
    type AlbumTrackRow,
} from "@spqtify/embed-image";
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

        return {
            // fixes type issues
            Vibrant: palette.Vibrant ? { hex: palette.Vibrant.hex } : undefined,
            DarkVibrant: palette.DarkVibrant
                ? { hex: palette.DarkVibrant.hex }
                : undefined,
        };
    });

export class GenerateSvgDataError extends Schema.TaggedError<GenerateSvgDataError>()(
    "GenerateSvgDataError",
    {},
) {}

export class GeneratePngError extends Schema.TaggedError<GeneratePngError>()(
    "GeneratePngError",
    {},
) {}

const ALBUM_IMAGE_WIDTH = 800;
const ALBUM_IMAGE_MIN_HEIGHT = 250;
const ALBUM_IMAGE_MAX_HEIGHT = 600;
const ALBUM_LIST_MAX_ROWS = 12;
const ALBUM_BASE_HEIGHT = 210;
const ALBUM_ROW_HEIGHT = 26;

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
                    width: ALBUM_IMAGE_WIDTH,
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

export const clampTrackIndex = (index: number, trackCount: number): number => {
    if (trackCount === 0) {
        return 0;
    }

    if (!Number.isFinite(index)) {
        return 0;
    }

    return Math.max(0, Math.min(Math.trunc(index), trackCount - 1));
};

export const buildAlbumTrackRows = (
    tracks: readonly string[],
    currentTrackIndex: number,
    maxRows = ALBUM_LIST_MAX_ROWS,
): { trackRows: AlbumTrackRow[]; overflowCount: number } => {
    if (tracks.length <= maxRows) {
        return {
            trackRows: tracks.map((title, index) => ({
                number: index + 1,
                title,
                isCurrent: index === currentTrackIndex,
            })),
            overflowCount: 0,
        };
    }

    const visibleTrackRows = Math.max(1, maxRows - 1);
    const maxStart = tracks.length - visibleTrackRows;
    const centeredStart = currentTrackIndex - Math.floor(visibleTrackRows / 2);
    const start = Math.max(0, Math.min(centeredStart, maxStart));
    const visibleTracks = tracks.slice(start, start + visibleTrackRows);
    const overflowCount = tracks.length - visibleTracks.length;

    return {
        trackRows: visibleTracks.map((title, index) => {
            const trackIndex = start + index;
            return {
                number: trackIndex + 1,
                title,
                isCurrent: trackIndex === currentTrackIndex,
            };
        }),
        overflowCount,
    };
};

export const calculateAlbumImageHeight = (
    visibleRows: number,
    hasOverflowRow: boolean,
): number => {
    const lineCount = visibleRows + (hasOverflowRow ? 1 : 0);
    const calculatedHeight = ALBUM_BASE_HEIGHT + lineCount * ALBUM_ROW_HEIGHT;
    return Math.max(
        ALBUM_IMAGE_MIN_HEIGHT,
        Math.min(calculatedHeight, ALBUM_IMAGE_MAX_HEIGHT),
    );
};

export const generateAlbumImage = (opts: {
    albumArt: ArrayBuffer;
    baseColor: string;
    gradientColor: string;
    titleText: string;
    artistText: string;
    tracks: readonly string[];
    currentTrackIndex: number;
}) =>
    Effect.gen(function* () {
        const normalizedTracks = opts.tracks
            .map((track) => track.trim())
            .filter((track) => track.length > 0);

        const clampedTrackIndex = clampTrackIndex(
            opts.currentTrackIndex,
            normalizedTracks.length,
        );
        const { trackRows, overflowCount } = buildAlbumTrackRows(
            normalizedTracks,
            clampedTrackIndex,
        );
        const imageHeight = calculateAlbumImageHeight(
            trackRows.length,
            overflowCount > 0,
        );

        const svgComp = OpenGraphAlbum({
            albumArt: opts.albumArt,
            baseColor: opts.baseColor,
            gradientColor: opts.gradientColor,
            titleText: opts.titleText,
            artistText: opts.artistText,
            trackRows,
            overflowCount,
            imageHeight,
        });

        const svgData = yield* Effect.tryPromise({
            try: () =>
                satori(svgComp, {
                    width: ALBUM_IMAGE_WIDTH,
                    height: imageHeight,
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
                const resvg = new Resvg(svgData, {
                    fitTo: {
                        mode: "width",
                        value: ALBUM_IMAGE_WIDTH,
                    },
                });
                return resvg.render().asPng();
            },
            catch: () => new GeneratePngError(),
        });

        return pngBuffer;
    });
