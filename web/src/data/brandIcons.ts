// Real brand glyphs (official SVG paths from simple-icons) mapped to AgentSpan's
// 52 channels. 41 channels resolve to a real logo here; the 10 that simple-icons
// can't supply (trademark-removed or nonexistent) get a hand-drawn vector glyph
// in customGlyphs.ts, and the single remaining one (xueqiu) falls back to a
// clean monogram in ChannelProfile. Net result: every channel shows a crisp
// vector mark — no bare letters next to logos. Each `path` is a 24×24 glyph.
import {
  siGithub, siYcombinator, siV2ex, siYoutube, siTiktok, siX, siReddit, siBilibili,
  siXiaohongshu, siInstagram, siWikipedia, siArxiv, siDiscord, siTelegram, siSpotify,
  siTwitch, siGooglescholar, siPodcastindex, siQuora, siPinterest, siNpm, siRust,
  siPypi, siGitlab, siDocker, siInternetarchive, siGooglemaps, siCoinbase, siDuckduckgo,
  siGooglenews, siStatuspage, siHuggingface, siAnthropic, siBrave, siGoogle, siNotion,
  siFlightaware, siDevdotto, siLobsters, siWikidata, siRss,
} from "simple-icons";

export interface BrandIcon {
  path: string;
  hex: string;
  title: string;
}

export const brandIcons: Record<string, BrandIcon> = {
  github: siGithub,
  hackernews: siYcombinator,
  v2ex: siV2ex,
  youtube: siYoutube,
  tiktok: siTiktok,
  twitter: siX,
  reddit: siReddit,
  bilibili: siBilibili,
  xiaohongshu: siXiaohongshu,
  instagram: siInstagram,
  wikipedia: siWikipedia,
  arxiv: siArxiv,
  discord: siDiscord,
  telegram: siTelegram,
  spotify: siSpotify,
  twitch: siTwitch,
  scholar: siGooglescholar,
  podcasts: siPodcastindex,
  quora: siQuora,
  pinterest: siPinterest,
  npm: siNpm,
  crates: siRust,
  pypi: siPypi,
  gitlab: siGitlab,
  dockerhub: siDocker,
  wayback: siInternetarchive,
  maps: siGooglemaps,
  coinbase: siCoinbase,
  duckduckgo: siDuckduckgo,
  gnews: siGooglenews,
  statuspage: siStatuspage,
  huggingface: siHuggingface,
  anthropic: siAnthropic,
  brave: siBrave,
  google: siGoogle,
  notion: siNotion,
  flight: siFlightaware,
  devto: siDevdotto,
  lobsters: siLobsters,
  wikidata: siWikidata,
  rss: siRss,
};
