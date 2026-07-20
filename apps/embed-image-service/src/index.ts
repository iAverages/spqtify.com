import { createServer } from "node:http";
import {
    HttpApi,
    HttpApiBuilder,
    HttpApiEndpoint,
    HttpApiGroup,
    HttpApp,
    HttpMiddleware,
    HttpServer,
    HttpServerResponse,
} from "@effect/platform";
import { NodeHttpServer, NodeRuntime } from "@effect/platform-node";
import { Effect, Layer, Schema } from "effect";
import {
    clampTrackIndex,
    GeneratePngError,
    GenerateSvgDataError,
    GetAlbumArtError,
    GetImagePaletteError,
    generateAlbumImage,
    generateImage,
    generatePlaylistImage,
    getArtworkArrayBuffer,
    getPaletteFromImage,
} from "./image";

const ServiceApi = HttpApi.make("service-api").add(
    HttpApiGroup.make("service-api")
        .add(HttpApiEndpoint.get("health", "/health").addSuccess(Schema.String))
        .add(
            HttpApiEndpoint.post("image-generation", "/image")
                .setPayload(
                    Schema.Struct({
                        albumArt: Schema.String,
                        songName: Schema.String,
                        artist: Schema.String,
                    }),
                )
                .addError(GetAlbumArtError, { status: 500 })
                .addError(GeneratePngError, { status: 500 })
                .addError(GenerateSvgDataError, { status: 500 })
                .addError(GetImagePaletteError, { status: 500 })
                .addSuccess(Schema.Uint8Array),
        )
        .add(
            HttpApiEndpoint.post("album-image-generation", "/image/album")
                .setPayload(
                    Schema.Struct({
                        albumArt: Schema.String,
                        titleText: Schema.String,
                        artistText: Schema.String,
                        tracks: Schema.Array(Schema.String),
                        currentTrackIndex: Schema.Number,
                    }),
                )
                .addError(GetAlbumArtError, { status: 500 })
                .addError(GeneratePngError, { status: 500 })
                .addError(GenerateSvgDataError, { status: 500 })
                .addError(GetImagePaletteError, { status: 500 })
                .addSuccess(Schema.Uint8Array),
        )
        .add(
            HttpApiEndpoint.post("playlist-image-generation", "/image/playlist")
                .setPayload(
                    Schema.Struct({
                        albumArt: Schema.String,
                        playlistName: Schema.String,
                        creatorName: Schema.String,
                        tracks: Schema.Array(Schema.String),
                        currentTrackIndex: Schema.Number,
                        trackCountIsCapped: Schema.Boolean,
                    }),
                )
                .addError(GetAlbumArtError, { status: 500 })
                .addError(GeneratePngError, { status: 500 })
                .addError(GenerateSvgDataError, { status: 500 })
                .addError(GetImagePaletteError, { status: 500 })
                .addSuccess(Schema.Uint8Array),
        ),
);

const ServiceApiImpl = HttpApiBuilder.group(
    ServiceApi,
    "service-api",
    (handlers) => {
        return handlers
            .handle("health", () => Effect.succeed("ok"))
            .handle("image-generation", (request) => {
                return Effect.gen(function* () {
                    const payload = request.payload;
                    const imageArrayBuffer = yield* getArtworkArrayBuffer(
                        payload.albumArt,
                    );

                    const palette =
                        yield* getPaletteFromImage(imageArrayBuffer);

                    const baseColor = palette.Vibrant?.hex ?? "";
                    const gradientColor = palette.DarkVibrant?.hex ?? "";

                    const pngBuffer = yield* generateImage({
                        ...payload,
                        albumArt: imageArrayBuffer,
                        baseColor,
                        gradientColor,
                    });

                    return HttpServerResponse.uint8Array(pngBuffer, {
                        contentType: "image/png",
                        headers: {
                            // send back the base color used on the
                            // html meta tags for the embed
                            "X-Basecolor": baseColor,
                        },
                    });
                });
            })
            .handle("album-image-generation", (request) => {
                console.log(request.payload);
                return Effect.gen(function* () {
                    const payload = request.payload;
                    const imageArrayBuffer = yield* getArtworkArrayBuffer(
                        payload.albumArt,
                    );

                    const palette =
                        yield* getPaletteFromImage(imageArrayBuffer);

                    const baseColor = palette.Vibrant?.hex ?? "";
                    const gradientColor = palette.DarkVibrant?.hex ?? "";

                    const pngBuffer = yield* generateAlbumImage({
                        albumArt: imageArrayBuffer,
                        baseColor,
                        gradientColor,
                        titleText: payload.titleText,
                        artistText: payload.artistText,
                        tracks: payload.tracks,
                        currentTrackIndex: clampTrackIndex(
                            payload.currentTrackIndex,
                            payload.tracks.length,
                        ),
                    });

                    return HttpServerResponse.uint8Array(pngBuffer, {
                        contentType: "image/png",
                        headers: {
                            "X-Basecolor": baseColor,
                        },
                    });
                });
            })
            .handle("playlist-image-generation", (request) => {
                return Effect.gen(function* () {
                    const payload = request.payload;
                    const imageArrayBuffer = yield* getArtworkArrayBuffer(
                        payload.albumArt,
                    );

                    const palette =
                        yield* getPaletteFromImage(imageArrayBuffer);

                    const baseColor = palette.Vibrant?.hex ?? "";
                    const gradientColor = palette.DarkVibrant?.hex ?? "";

                    const pngBuffer = yield* generatePlaylistImage({
                        albumArt: imageArrayBuffer,
                        baseColor,
                        gradientColor,
                        playlistName: payload.playlistName,
                        creatorName: payload.creatorName,
                        tracks: payload.tracks,
                        trackCountIsCapped: payload.trackCountIsCapped,
                        currentTrackIndex: clampTrackIndex(
                            payload.currentTrackIndex,
                            payload.tracks.length,
                        ),
                    });

                    return HttpServerResponse.uint8Array(pngBuffer, {
                        contentType: "image/png",
                        headers: {
                            "X-Basecolor": baseColor,
                        },
                    });
                });
            });
    },
);

const ServiceApiLive = HttpApiBuilder.api(ServiceApi).pipe(
    Layer.provide(ServiceApiImpl),
);

const Server = HttpApiBuilder.serve((httpApp) =>
    HttpMiddleware.logger(
        HttpApp.withPreResponseHandler(httpApp, (_, response) => {
            if (response.status !== 404) {
                return Effect.succeed(response);
            }

            return Effect.succeed(
                HttpServerResponse.unsafeJson(
                    { message: "invalid route" },
                    { status: 404 },
                ),
            );
        }),
    ),
).pipe(
    Layer.provide(ServiceApiLive),
    HttpServer.withLogAddress,
    Layer.provide(NodeHttpServer.layer(createServer, { port: 3001 })),
);

Layer.launch(Server).pipe(NodeRuntime.runMain);
