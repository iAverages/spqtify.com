import assert from "node:assert/strict";
import test from "node:test";
import { buildAlbumTrackRows } from "../src/track-rows";

test("marks the overflow count as a lower bound for capped track lists", () => {
	const tracks = Array.from({ length: 100 }, (_, index) => `Track ${index + 1}`);

	const result = buildAlbumTrackRows(tracks, 0, true);

	assert.deepEqual(
		{
			visibleTracks: result.trackRows.length,
			overflowCount: result.overflowCount,
			overflowCountIsMinimum: result.overflowCountIsMinimum,
		},
		{
			visibleTracks: 11,
			overflowCount: 89,
			overflowCountIsMinimum: true,
		},
	);
});

test("falls back to the embedded track count", () => {
	const tracks = Array.from({ length: 100 }, (_, index) => `Track ${index + 1}`);

	const result = buildAlbumTrackRows(tracks, 0);

	assert.deepEqual(
		{
			overflowCount: result.overflowCount,
			overflowCountIsMinimum: result.overflowCountIsMinimum,
		},
		{
			overflowCount: 89,
			overflowCountIsMinimum: false,
		},
	);
});
