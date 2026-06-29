// Hand-drawn 24×24 vector glyphs for the channels simple-icons can't supply —
// big brands it dropped for trademark (linkedin, slack, bing, openai) and
// services with no official mark (exa, weather, xiaoyuzhou, openlibrary,
// gutenberg, web). Each is a single white `d` path placed on the channel
// silhouette exactly like a real logo, so the network grid reads uniformly.

export interface CustomGlyph {
  path: string;
  fillRule?: "evenodd" | "nonzero";
}

export const customGlyphs: Record<string, CustomGlyph> = {
  // generic web — a wireframe globe (ring + equator + meridian)
  web: {
    path:
      "M12 2A10 10 0 1 0 12 22A10 10 0 1 0 12 2Z M12 4A8 8 0 1 1 12 20A8 8 0 1 1 12 4Z " +
      "M4 11.1H20V12.9H4Z " +
      "M12 4A4.6 8 0 1 0 12 20A4.6 8 0 1 0 12 4Z M12 5.6A3 8 0 1 1 12 18.4A3 8 0 1 1 12 5.6Z",
    fillRule: "evenodd",
  },

  // linkedin — lowercase "in" lettermark
  linkedin: {
    path:
      "M5.6 4A1.5 1.5 0 1 0 5.6 7A1.5 1.5 0 1 0 5.6 4Z " +
      "M4.2 8.5H7V20H4.2Z " +
      "M9.2 8.5H11.9V10.1C12.6 9 13.8 8.3 15.4 8.3C18 8.3 19.8 9.9 19.8 13.2V20H17.1V13.7" +
      "C17.1 12 16.4 10.9 14.8 10.9C13.3 10.9 11.9 11.9 11.9 13.9V20H9.2Z",
  },

  // slack — four pinwheeled pills
  slack: {
    path:
      "M13.4 4.1A1.6 1.6 0 0 1 16.6 4.1V7.9A1.6 1.6 0 0 1 13.4 7.9Z " +
      "M16.1 13.4H19.9A1.6 1.6 0 0 1 19.9 16.6H16.1A1.6 1.6 0 0 1 16.1 13.4Z " +
      "M7.4 16.1A1.6 1.6 0 0 1 10.6 16.1V19.9A1.6 1.6 0 0 1 7.4 19.9Z " +
      "M4.1 7.4H7.9A1.6 1.6 0 0 1 7.9 10.6H4.1A1.6 1.6 0 0 1 4.1 7.4Z",
  },

  // openai — four-petal bloom
  openai: {
    path:
      "M12 2.5A2 4.5 0 1 0 12 11.5A2 4.5 0 1 0 12 2.5Z " +
      "M12 12.5A2 4.5 0 1 0 12 21.5A2 4.5 0 1 0 12 12.5Z " +
      "M2.5 12A4.5 2 0 1 0 11.5 12A4.5 2 0 1 0 2.5 12Z " +
      "M12.5 12A4.5 2 0 1 0 21.5 12A4.5 2 0 1 0 12.5 12Z " +
      "M9 12A3 3 0 1 0 15 12A3 3 0 1 0 9 12Z",
  },

  // bing — magnifier
  bing: {
    path:
      "M10 4A6 6 0 1 0 10 16A6 6 0 1 0 10 4Z M10 6.4A3.6 3.6 0 1 1 10 13.6A3.6 3.6 0 1 1 10 6.4Z " +
      "M14.4 16.1L16.1 14.4L21 19.3A1.2 1.2 0 0 1 19.3 21Z",
    fillRule: "evenodd",
  },

  // exa — sparkle (search + AI)
  exa: {
    path:
      "M10.5 2C11.2 7 12.5 8.3 17.5 9C12.5 9.7 11.2 11 10.5 16C9.8 11 8.5 9.7 3.5 9C8.5 8.3 9.8 7 10.5 2Z " +
      "M18 13C18.3 15 18.6 15.4 20.5 15.7C18.6 16 18.3 16.4 18 18.4C17.7 16.4 17.4 16 15.5 15.7C17.4 15.4 17.7 15 18 13Z",
  },

  // weather — sun with rays
  weather: {
    path:
      "M12 7.5A4.5 4.5 0 1 0 12 16.5A4.5 4.5 0 1 0 12 7.5Z " +
      "M11 2H13V4.6H11Z M11 19.4H13V22H11Z M2 11H4.6V13H2Z M19.4 11H22V13H19.4Z " +
      "M16.9 5.7L18.3 7.1L16.6 8.8L15.2 7.4Z M7.1 5.7L5.7 7.1L7.4 8.8L8.8 7.4Z " +
      "M16.9 18.3L18.3 16.9L16.6 15.2L15.2 16.6Z M7.1 18.3L5.7 16.9L7.4 15.2L8.8 16.6Z",
  },

  // xiaoyuzhou — microphone (podcast app)
  xiaoyuzhou: {
    path:
      "M12 3A2.5 2.5 0 0 1 14.5 5.5V11A2.5 2.5 0 0 1 9.5 11V5.5A2.5 2.5 0 0 1 12 3Z " +
      "M7 10.5A5 5 0 0 0 17 10.5H15.2A3.2 3.2 0 0 1 8.8 10.5Z " +
      "M11 16.4H13V20H11Z M8.5 20H15.5V21.8H8.5Z",
  },

  // openlibrary — open book
  openlibrary: {
    path:
      "M3 4.8C3 4.2 3.5 3.9 4.1 4L11 5.4V20.2L4.1 18.8C3.5 18.7 3 18.2 3 17.6Z " +
      "M21 4.8C21 4.2 20.5 3.9 19.9 4L13 5.4V20.2L19.9 18.8C20.5 18.7 21 18.2 21 17.6Z",
  },

  // gutenberg — closed book with a bookmark
  gutenberg: {
    path:
      "M6 3H17.5A1.5 1.5 0 0 1 19 4.5V20A1 1 0 0 1 17.5 20.9H6A2 2 0 0 1 4 18.9V5A2 2 0 0 1 6 3Z " +
      "M13 3H15.5V8.5L14.25 7.2L13 8.5Z",
    fillRule: "evenodd",
  },
};
