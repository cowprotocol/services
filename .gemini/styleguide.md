# Code Review Style Guide: Stricter & Actionable Feedback Only

## 1. Interaction & Noise Reduction (Silent Mode)
* **No Positive Feedback:** Do not post comments that praise the code, highlight "good solutions," or confirm that a change is a "good improvement."
* **Silent on Success:** If no functional bugs, security vulnerabilities, or significant performance regressions are found, do not post any comments at all.
* **Severity Threshold:** Only surface findings that meet the "High" or "Critical" severity criteria. Treat "Medium" findings as "Low" and ignore them unless they represent a direct logic error.

## 2. Technical Standards
* **Actionability:** Every comment must include a clear, actionable suggestion for improvement. Do not leave "FYI" or informational comments.
* **Edge Cases & Logic:** Focus exclusively on identifying missing edge cases, potential race conditions, or logic that deviates from the PR's stated goals.
* **Security & Performance:** Prioritize findings related to resource leaks, unnecessary complexity, or potential security vulnerabilities.

## 3. Formatting
* **Conciseness:** Keep comments brief and technical. Skip the introductory and concluding pleasantries.
