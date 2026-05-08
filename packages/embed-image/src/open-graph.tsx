import type { ReactNode } from "react";

const truncateText = (value: string, maxChars: number): string => {
    const trimmed = value.trim();
    if (trimmed.length <= maxChars) {
        return trimmed;
    }
    return `${trimmed.slice(0, Math.max(0, maxChars - 1)).trimEnd()}...`;
};

export type OpenGraphTrackProps = {
    baseColor: string;
    gradientColor: string;
    albumArt: ArrayBuffer;
    songName: string;
    artist: string;
};

export type AlbumTrackRow = {
    number: number;
    title: string;
    isCurrent: boolean;
};

export type OpenGraphAlbumProps = {
    baseColor: string;
    gradientColor: string;
    albumArt: ArrayBuffer;
    titleText: string;
    artistText: string;
    trackRows: AlbumTrackRow[];
    overflowCount: number;
    imageHeight: number;
};

export const OpenGraph = ({
    gradientColor,
    baseColor,
    albumArt,
    songName,
    artist,
}: OpenGraphTrackProps): ReactNode => {
    const safeSongName = truncateText(songName, 44);
    const safeArtist = truncateText(artist, 36);

    return (
        <div
            style={{
                borderRadius: "12px",
                height: "100%",
                width: "100%",
                display: "flex",
                flexDirection: "row",
                alignItems: "center",
                justifyContent: "center",
                background: `linear-gradient(45deg, ${baseColor}, ${gradientColor})`,
                fontFamily: "system-ui",
                padding: "20px",
            }}
        >
            <img
                // satori allows for ArrayBuffer, react type does not
                src={albumArt as unknown as string}
                alt="Album Art"
                style={{
                    width: "250px",
                    height: "250px",
                    marginRight: "30px",
                    borderRadius: "12px",
                    boxShadow: "0 4px 8px rgba(0, 0, 0, 0.3)",
                }}
            />

            <div
                style={{
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "flex-start",
                    justifyContent: "center",
                    maxWidth: "60%",
                }}
            >
                <h1
                    style={{
                        fontSize: "40px",
                        color: "white",
                        margin: "0",
                    }}
                >
                    {safeSongName}
                </h1>
                <h2
                    style={{
                        fontSize: "32px",
                        color: "rgba(255, 255, 255, 0.8)",
                        margin: "10px 0",
                    }}
                >
                    {safeArtist}
                </h2>
            </div>

            <SpotifyIcon />
        </div>
    );
};

export const OpenGraphAlbum = ({
    gradientColor,
    baseColor,
    albumArt,
    titleText,
    artistText,
    trackRows,
    overflowCount,
    imageHeight,
}: OpenGraphAlbumProps): ReactNode => {
    const safeTitleText = truncateText(titleText, 42);
    const safeArtistText = truncateText(artistText, 40);

    return (
        <div
            style={{
                borderRadius: "12px",
                height: `${imageHeight}px`,
                width: "800px",
                display: "flex",
                flexDirection: "row",
                alignItems: "stretch",
                background: `linear-gradient(45deg, ${baseColor}, ${gradientColor})`,
                fontFamily: "system-ui",
                padding: "20px",
                boxSizing: "border-box",
                position: "relative",
            }}
        >
            <div
                style={{
                    width: "220px",
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "center",
                    justifyContent: "flex-start",
                    marginRight: "20px",
                    flexShrink: 0,
                }}
            >
                <img
                    src={albumArt as unknown as string}
                    alt="Album Art"
                    style={{
                        width: "220px",
                        height: "220px",
                        borderRadius: "12px",
                        boxShadow: "0 4px 8px rgba(0, 0, 0, 0.3)",
                    }}
                />
            </div>

            <div
                style={{
                    display: "flex",
                    flexDirection: "column",
                    flexGrow: 1,
                    minWidth: 0,
                }}
            >
                <div
                    style={{
                        marginBottom: "14px",
                        display: "flex",
                        flexDirection: "column",
                        minWidth: 0,
                    }}
                >
                    <h1
                        style={{
                            display: "block",
                            width: "100%",
                            fontSize: "34px",
                            color: "white",
                            margin: "0",
                            lineHeight: 1.1,
                            whiteSpace: "nowrap",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                        }}
                    >
                        {safeTitleText}
                    </h1>
                    <h2
                        style={{
                            display: "block",
                            width: "100%",
                            fontSize: "24px",
                            color: "rgba(255, 255, 255, 0.8)",
                            margin: "6px 0 0",
                            lineHeight: 1.2,
                            whiteSpace: "nowrap",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                        }}
                    >
                        {safeArtistText}
                    </h2>
                </div>

                <div
                    style={{
                        display: "flex",
                        flexDirection: "column",
                        gap: "6px",
                        minWidth: 0,
                    }}
                >
                    {trackRows.map((track) => (
                        <div
                            key={`${track.number}-${track.title}`}
                            style={{
                                display: "flex",
                                flexDirection: "row",
                                alignItems: "center",
                                minWidth: 0,
                                borderRadius: "8px",
                                padding: "3px 10px",
                                backgroundColor: track.isCurrent
                                    ? "rgba(255, 255, 255, 0.2)"
                                    : "rgba(0, 0, 0, 0.12)",
                            }}
                        >
                            <span
                                style={{
                                    width: "36px",
                                    fontSize: "17px",
                                    color: track.isCurrent
                                        ? "white"
                                        : "rgba(255, 255, 255, 0.75)",
                                    flexShrink: 0,
                                }}
                            >
                                {`${track.number.toString().padStart(2, "0")}.`}
                            </span>
                            <span
                                style={{
                                    fontSize: "17px",
                                    color: "white",
                                    whiteSpace: "nowrap",
                                    overflow: "hidden",
                                    textOverflow: "ellipsis",
                                    minWidth: 0,
                                    lineHeight: 1.25,
                                }}
                            >
                                {track.title}
                            </span>
                        </div>
                    ))}

                    {overflowCount > 0 ? (
                        <div
                            style={{
                                fontSize: "15px",
                                color: "rgba(255, 255, 255, 0.8)",
                                padding: "0 10px",
                            }}
                        >
                            {`...and ${overflowCount} more`}
                        </div>
                    ) : null}
                </div>
            </div>

            <SpotifyIcon />
        </div>
    );
};

const SpotifyIcon = () => {
    return (
        <svg
            width="40"
            height="40"
            viewBox="0 0 24 24"
            fill="white"
            style={{
                position: "absolute",
                bottom: "20px",
                right: "20px",
            }}
        >
            <path d="M12 0C5.4 0 0 5.4 0 12s5.4 12 12 12 12-5.4 12-12S18.66 0 12 0zm5.521 17.34c-.24.359-.66.48-1.021.24-2.82-1.74-6.36-2.101-10.561-1.141-.418.122-.779-.179-.899-.539-.12-.421.18-.78.54-.9 4.56-1.021 8.52-.6 11.64 1.32.42.18.479.659.301 1.02zm1.44-3.3c-.301.42-.841.6-1.262.3-3.239-1.98-8.159-2.58-11.939-1.38-.479.12-1.02-.12-1.14-.6-.12-.48.12-1.021.6-1.141C9.6 9.9 15 10.561 18.72 12.84c.361.181.54.78.241 1.2zm.12-3.36C15.24 8.4 8.82 8.16 5.16 9.301c-.6.179-1.2-.181-1.38-.721-.18-.601.18-1.2.72-1.381 4.26-1.26 11.28-1.02 15.721 1.621.539.3.719 1.02.419 1.56-.299.421-1.02.599-1.559.3z" />
        </svg>
    );
};
