import tsParser from "@typescript-eslint/parser";

function hasTranslatableText(value) {
  const normalized = value.replace(/\s+/g, " ").trim();
  return normalized.length > 0 && /[\p{L}\p{N}]/u.test(normalized);
}

const i18next = {
  rules: {
    "no-literal-string": {
      meta: {
        type: "problem",
        docs: {
          description: "Require JSX text to come from i18n translations.",
        },
        messages: {
          literal: "Move this user-facing string to i18n translations.",
        },
        schema: [],
      },
      create(context) {
        return {
          JSXText(node) {
            if (hasTranslatableText(node.value)) {
              context.report({ node, messageId: "literal" });
            }
          },
        };
      },
    },
  },
};

export default [
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaFeatures: {
          jsx: true,
        },
      },
    },
    plugins: {
      i18next,
    },
    rules: {
      // Catch text in JSX that should be translated
      "i18next/no-literal-string": "error",
    },
  },
];
