import {
    HttpMiddleware,
    HttpRouter,
    HttpServer,
    HttpServerResponse,
} from "@effect/platform";
import { NodeHttpServer, NodeRuntime } from "@effect/platform-node";
import { Effect, Layer } from "effect";
import { createServer } from "node:http";

const router = HttpRouter.empty.pipe(
    HttpRouter.get("/", HttpServerResponse.text("meow")),
    HttpRouter.get(
        "/og",
        Effect.gen(function* () {
            return yield* HttpServerResponse.json({ id: "1", name: "Alice" });
        }),
    ),
);

const app = router.pipe(
    Effect.catchTags({
        RouteNotFound: () =>
            HttpServerResponse.json(
                { message: "invalid route" },
                { status: 404 },
            ),
    }),
    Effect.catchAllCause((cause) =>
        HttpServerResponse.json({ message: cause.toString() }, { status: 500 }),
    ),

    HttpServer.serve(HttpMiddleware.logger),
    HttpServer.withLogAddress,
);

const ServerLive = NodeHttpServer.layer(() => createServer(), { port: 3001 });

NodeRuntime.runMain(Layer.launch(Layer.provide(app, ServerLive)));
