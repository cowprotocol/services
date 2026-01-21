# Code Review Style Guide: Stricter & Actionable Feedback Only

## 1. Interaction & Noise Reduction
* **No Positive Feedback:** Do not post comments that praise the code, highlight "good solutions," or confirm that a change is a "good improvement."
* **Confirm Clean Reviews:** If no critical issues are found, post a single summary comment stating "No critical issues found" rather than staying silent.
* **Severity Threshold:** Only surface findings that meet the "High" or "Critical" severity criteria. Treat "Medium" findings as "Low" and ignore them unless they represent a direct logic error.
* **Focus on Changed Code:** Only comment on code that was actually modified in the PR. Do not flag issues in unchanged code or code that was simply moved/refactored without logic changes, unless the issue is a severe security vulnerability or critical bug.

## 2. Technical Standards
* **Actionability:** Every comment must include a clear, actionable suggestion for improvement. Do not leave "FYI" or informational comments.
* **Edge Cases & Logic:** Focus exclusively on identifying missing edge cases, potential race conditions, or logic that deviates from the PR's stated goals.
* **Security & Performance:** Prioritize findings related to resource leaks, unnecessary complexity, or potential security vulnerabilities.

## 3. Formatting
* **Conciseness:** Keep comments concise, direct and technical. Skip the introductory and concluding pleasantries.
