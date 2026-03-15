import type { MetadataRoute } from "next";

export default function sitemap(): MetadataRoute.Sitemap {
  const baseUrl = "https://maxc.polluxstudio.in";

  return [
    {
      url: baseUrl,
      lastModified: new Date(),
    },
    {
      url: `${baseUrl}/docs`,
      lastModified: new Date(),
    },
    {
      url: `${baseUrl}/downloads`,
      lastModified: new Date(),
    },
    {
      url: `${baseUrl}/github`,
      lastModified: new Date(),
    },
  ];
}
