import type { ImageMetadata } from 'astro';

/**
A screenshot of the demo, as `astro:assets` imports it.
*/
export type Still = {
  still: ImageMetadata;
};

/**
A video of the demo by its name in `public/media/`, as WebM and MP4.
*/
export type Video = {
  video: string;
};
