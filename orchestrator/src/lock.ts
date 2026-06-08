import { type CodebaseTopology } from "./schema.js";

/**
 * Patches the existing README markdown content by updating only the automated blocks.
 * If a block boundary does not exist in the file, it is cleanly appended to the end.
 * If the file is completely empty, a new skeleton layout is created.
 */
export function patchReadme(
  existingContent: string,
  sections: Record<string, string>,
): string {
  const normalizedContent = existingContent.trim();

  if (normalizedContent === "") {
    // Generate new skeleton layout if empty
    let newDoc = "# Codebase Documentation\n\n";

    for (const [key, val] of Object.entries(sections)) {
      newDoc += `<!-- NEXUS_START:${key} -->\n${val}\n<!-- NEXUS_END:${key} -->\n\n`;
    }
    return newDoc.trim() + "\n";
  }

  let patched = normalizedContent;

  for (const [key, val] of Object.entries(sections)) {
    // Spacing-agnostic, OS-independent line ending regex matching
    const sectionRegex = new RegExp(
      `(<!--\\s*NEXUS_START:${key}\\s*-->)([\\s\\S]*?)(<!--\\s*NEXUS_END:${key}\\s*-->)`,
      "i",
    );

    if (sectionRegex.test(patched)) {
      // Replace only the inner contents, keeping the boundaries intact
      patched = patched.replace(sectionRegex, `$1\n${val}\n$3`);
    } else {
      // Append section at the end of the document if marker is missing
      const newlinePrefix = patched.endsWith("\n") ? "\n" : "\n\n";
      patched += `${newlinePrefix}<!-- NEXUS_START:${key} -->\n${val}\n<!-- NEXUS_END:${key} -->\n`;
    }
  }

  // Ensure file ends with a single trailing newline
  return patched.trim() + "\n";
}
