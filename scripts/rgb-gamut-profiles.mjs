import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

// Use the production Rust profile coefficients and conversion probes. No copy
// of RGB matrices is maintained in these development-time JS generators.
export async function loadProfiles() {
	const { stdout } = await promisify(execFile)("cargo", [
		"run", "--quiet", "--manifest-path", "rust/Cargo.toml", "--example", "rgb-gamut-profiles",
	], { cwd: fileURLToPath(new URL("../", import.meta.url)), encoding: "utf8" });
	return JSON.parse(stdout);
}
