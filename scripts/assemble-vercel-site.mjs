import { cpSync, mkdirSync, rmSync } from "node:fs";

rmSync("public", { recursive: true, force: true });
mkdirSync("public", { recursive: true });
cpSync("website", "public", {
  recursive: true,
  filter: (src) => {
    const base = src.split("/").pop();
    return base !== ".vercel" && base !== ".gitignore" && base !== "vercel.json";
  },
});
// Shared notes and folders (/s/, /f/) link to /style.css.
cpSync("sharing/public/style.css", "public/style.css");
