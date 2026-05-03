import {
    HttpApp,
    HttpApi,
    HttpApiBuilder,
    HttpApiEndpoint,
    HttpApiGroup,
    HttpMiddleware,
    HttpServer,
    HttpServerResponse,
} from "@effect/platform";
import { NodeHttpServer, NodeRuntime } from "@effect/platform-node";
import { Effect, Layer, Schema } from "effect";
import { createServer } from "node:http";
import {
    generateImage,
    GeneratePngError,
    GenerateSvgDataError,
    GetAlbumArtError,
    getArtworkArrayBuffer,
    GetImagePaletteError,
    getPaletteFromImage,
} from "./image";

const ServiceApi = HttpApi.make("service-api").add(
    HttpApiGroup.make("service-api").add(
        HttpApiEndpoint.post("image-generation", "/image")
            .setPayload(
                Schema.Struct({
                    baseColor: Schema.String,
                    gradientColor: Schema.String,
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
    ),
);

const ServiceApiImpl = HttpApiBuilder.group(
    ServiceApi,
    "service-api",
    (handlers) => {
        return handlers.handle("image-generation", (request) => {
            return Effect.gen(function* () {
                const payload = request.payload;
                const imageArrayBuffer = yield* getArtworkArrayBuffer(
                    payload.albumArt,
                );

                const palette = yield* getPaletteFromImage(imageArrayBuffer);

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
