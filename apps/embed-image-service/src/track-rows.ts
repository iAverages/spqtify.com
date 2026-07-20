export type TrackRow = {
	number: number;
	title: string;
	isCurrent: boolean;
};

const MAX_ROWS = 12;

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
	trackCountIsCapped = false,
	maxRows = MAX_ROWS,
): {
	trackRows: TrackRow[];
	overflowCount: number;
	overflowCountIsMinimum: boolean;
} => {
	if (tracks.length <= maxRows) {
		return {
			trackRows: tracks.map((title, index) => ({
				number: index + 1,
				title,
				isCurrent: index === currentTrackIndex,
			})),
			overflowCount: 0,
			overflowCountIsMinimum: false,
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
		overflowCountIsMinimum: trackCountIsCapped,
	};
};
