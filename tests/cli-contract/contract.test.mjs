// CLI contract tests (specs#testing, requirements/quality#Tests): they run the compiled `weaver-cli`
// binary and check, for every command of the registry, `--help` and `--help --json`, input
// validation (exit 2 with field paths), exit codes 0 and 1, the shape of the Markdown and `--json`
// output, and `--input -` with and without a UTF-8 BOM.
//
// Run with `node --test tests/cli-contract/` after building `weaver-cli` (path in `CLI_CONTRACT_BIN`,
// else `target/release/weaver-cli`, else `target/debug/weaver-cli`).
//
// The command registry is built by the Task "Riga di comando weaver". Until `weaver --help` lists
// the objects of `requirements/cli#General`, the per-command tests are skipped and say why; from
// the first object listed, every command of the table below is tested and a missing one fails.

import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import { after, describe, test } from "node:test";

import { WEAVER_CLI, cleanup, entries, git, gitRepository, hermeticEnvironment, makeFolder, newProject, run, temporaryFolder } from "./support.mjs";

after(cleanup);

/**
 * The commands of `requirements/cli#General`, with the positional arguments a call needs: the
 * card commands take an identity (`aaaaaaaa` is never a live card) and `move` takes a state.
 */
export const COMMANDS = [
  ["model", "show", ["system"]],
  ["model", "list", []],
  ["model", "check", []],
  ["project", "new", []],
  ["project", "edit", []],
  ["project", "show", []],
  ["proposal", "new", []],
  ["proposal", "show", ["aaaaaaaa"]],
  ["proposal", "list", []],
  ["proposal", "edit", ["aaaaaaaa"]],
  ["proposal", "move", ["aaaaaaaa", "open"]],
  ["task", "new", []],
  ["task", "show", ["aaaaaaaa"]],
  ["task", "list", []],
  ["task", "move", ["aaaaaaaa", "doing"]],
  ["task", "delete", ["aaaaaaaa"]],
  ["drift", "new", []],
  ["drift", "show", ["aaaaaaaa"]],
  ["drift", "list", []],
  ["drift", "edit", ["aaaaaaaa"]],
  ["drift", "move", ["aaaaaaaa", "dismissed"]],
  ["history", "new", []],
  ["history", "edit", ["aaaaaaaa"]],
  ["history", "show", ["aaaaaaaa"]],
  ["history", "list", []],
  ["announcement", "new", []],
  ["job", "new", []],
  ["job", "show", ["aaaaaaaa"]],
  ["job", "list", []],
  ["job", "edit", []],
  ["question", "new", []],
  ["request", "new", []],
];
const OBJECTS = [...new Set(COMMANDS.map(([object]) => object))];

/** Commands that only read, and answer exit 0 in a fresh project with no input. */
const READS = [
  ["model", "list"],
  ["model", "check"],
  ["project", "show"],
  ["proposal", "list"],
  ["task", "list"],
  ["drift", "list"],
  ["history", "list"],
];
/** Commands that look a card up by name: an unknown name is refused with exit 1. */
const LOOKUPS = [
  ["proposal", "show", "p"],
  ["task", "show", "t"],
  ["drift", "show", "d"],
  ["history", "show", null],
  ["job", "show", null],
];

// ---------------------------------------------------------------------------------------------
// Helpers

const environment = hermeticEnvironment();
const exec = (args, options = {}) => run(args, { env: environment.env, ...options });
const emptyFolder = () => temporaryFolder("cli-contract-empty-");

/** Every file under `root` with its content: a refused command must leave this unchanged. */
function snapshot(root) {
  const files = {};
  const visit = (folder) => {
    for (const name of readdirSync(folder)) {
      const full = path.join(folder, name);
      if (statSync(full).isDirectory()) visit(full);
      else files[path.relative(root, full)] = readFileSync(full).toString("base64");
    }
  };
  if (existsSync(root)) visit(root);
  return files;
}

/** Whole text parsed as JSON, or `undefined`. */
function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

/** Every card in a `--json` output has `id` and `number` (`null` without a live number). */
function assertCardsHaveNumbers(value, where = "$") {
  if (Array.isArray(value)) value.forEach((item, i) => assertCardsHaveNumbers(item, `${where}[${i}]`));
  else if (value !== null && typeof value === "object") {
    if ("id" in value) {
      assert.ok("number" in value, `${where} has "id" but no "number"`);
      assert.ok(value.number === null || Number.isInteger(value.number), `${where}.number is not an integer or null`);
    }
    for (const [key, item] of Object.entries(value)) assertCardsHaveNumbers(item, `${where}.${key}`);
  }
}

/** A value of another JSON type than the schema's, for the type check of the input. */
function wrongTypeFor(schema) {
  const type = Array.isArray(schema.type) ? schema.type[0] : schema.type;
  return type === "string" ? 123 : type === "integer" || type === "number" ? "text" : type === "boolean" ? "text" : type === "array" ? "text" : type === "object" ? "text" : undefined;
}

// ---------------------------------------------------------------------------------------------
// What exists whatever the registry: the top-level help and the hermetic run itself.

describe("weaver --help", () => {
  test("exits 0, prints usage on stdout and reads no stdin", async () => {
    const cwd = emptyFolder();
    const result = await exec(["--help"], { cwd, stdin: "open" });
    assert.equal(result.timedOut, false, "--help waited for stdin");
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /Usage:\s+weaver\b/, "the usage line names `weaver`");
    assert.match(result.stdout, /-h, --help/);
    assert.equal(result.stderr, "");
  });

  test("-h prints the same help as --help", async () => {
    const cwd = emptyFolder();
    const long = await exec(["--help"], { cwd });
    const short = await exec(["-h"], { cwd });
    assert.equal(short.status, 0);
    assert.ok(short.stdout.length > 0 && long.stdout.length >= short.stdout.length);
    assert.match(short.stdout, /Usage:\s+weaver\b/);
  });

  test("creates nothing in the folder, the home or the data folders", async () => {
    const cwd = emptyFolder();
    const before = entries(environment.sandbox);
    await exec(["--help"], { cwd });
    assert.deepEqual(entries(cwd), [], "no .weaver or other file in the current folder");
    assert.deepEqual(entries(environment.sandbox), before, "nothing written in the sandboxed home");
  });

  test("prints the same with or without WEAVER_* variables", async () => {
    const cwd = emptyFolder();
    const clean = await exec(["--help"], { cwd });
    const polluted = await run(["--help"], {
      cwd,
      env: { ...environment.env, WEAVER_ENDPOINT: "not-an-endpoint", WEAVER_LOG: "debug", WEAVER_ROOT: "/nonexistent" },
    });
    assert.equal(polluted.status, clean.status);
    assert.equal(polluted.stdout, clean.stdout);
    assert.equal(polluted.stderr, clean.stderr);
  });

  test("an unknown option is wrong usage: exit 2, message on stderr, nothing on stdout", async () => {
    const result = await exec(["--no-such-option"], { cwd: emptyFolder() });
    assert.equal(result.status, 2);
    assert.equal(result.stdout, "");
    assert.ok(result.stderr.trim().length > 0);
  });

  test("a wrong usage creates nothing", async () => {
    const cwd = emptyFolder();
    await exec(["--no-such-option"], { cwd });
    assert.deepEqual(entries(cwd), []);
  });
});

// ---------------------------------------------------------------------------------------------
// The registry: from here on, one block per command.

const topHelp = await exec(["--help"], { cwd: emptyFolder() });
const listed = OBJECTS.filter((object) => new RegExp(`^\\s+${object}\\b`, "m").test(topHelp.stdout));
const registryBuilt = listed.length > 0;
const SKIP = registryBuilt
  ? false
  : "the command registry is not built yet (Task \"Riga di comando weaver\"): `weaver --help` lists none of the objects of requirements/cli#General";

describe("command registry", { skip: SKIP }, () => {
  test("`weaver --help` lists every object of requirements/cli#General", () => {
    assert.deepEqual(listed, OBJECTS);
  });
});

describe("every command", { skip: SKIP }, () => {
  for (const [object, verb, positional] of COMMANDS) {
    const name = `${object} ${verb}`;
    const withArgs = (...extra) => [object, verb, ...positional, ...extra];

    describe(name, () => {
      /** The input schema, from `--help --json`, read once. */
      let inputSchema;
      const schema = async () => {
        if (!inputSchema) {
          const result = await exec([object, verb, "--help", "--json"], { cwd: emptyFolder() });
          inputSchema = parseJson(result.stdout)?.input ?? {};
        }
        return inputSchema;
      };

      test("--help exits 0 with usage, reads no stdin and writes nothing", async () => {
        const cwd = emptyFolder();
        const result = await exec([object, verb, "--help"], { cwd, stdin: "open" });
        assert.equal(result.timedOut, false, "--help waited for stdin");
        assert.equal(result.status, 0, result.stderr);
        assert.ok(result.stdout.includes(`weaver ${object} ${verb}`), "usage names the command");
        assert.deepEqual(entries(cwd), []);
      });

      test("--help --json prints {command, input, output} JSON Schemas (2020-12)", async () => {
        const result = await exec([object, verb, "--help", "--json"], { cwd: emptyFolder(), stdin: "open" });
        assert.equal(result.timedOut, false, "--help --json waited for stdin");
        assert.equal(result.status, 0, result.stderr);
        const help = parseJson(result.stdout);
        assert.ok(help && typeof help === "object", "stdout is one JSON object");
        assert.deepEqual(Object.keys(help).sort(), ["command", "input", "output"]);
        for (const key of ["command", "input", "output"]) {
          assert.equal(typeof help[key], "object", `${key} is a JSON Schema object`);
          assert.match(String(help[key].$schema), /2020-12/, `${key} declares JSON Schema 2020-12`);
        }
      });

      if (name !== "project new") {
        test("without .weaver/ it fails with exit 1 saying to initialise the project", async () => {
          const cwd = emptyFolder();
          const result = await exec(withArgs("--root", cwd, "--input", "{}"), { cwd });
          assert.equal(result.status, 1, `${result.stdout}${result.stderr}`);
          assert.match(result.stderr, /initiali[sz]e/i);
          assert.deepEqual(entries(cwd), [], "a refused command creates no folder");
        });

        test("a folder with only an old .trama/ is not a project: exit 1 saying to initialise it", async () => {
          const cwd = emptyFolder();
          makeFolder(cwd, ".trama/model");
          writeFileSync(path.join(cwd, ".trama", "model", "system.md"), "# Old\n");
          const before = snapshot(cwd);
          const result = await exec(withArgs("--root", cwd, "--input", "{}"), { cwd });
          assert.equal(result.status, 1, `${result.stdout}${result.stderr}`);
          assert.match(result.stderr, /initiali[sz]e/i);
          assert.deepEqual(snapshot(cwd), before, "a refused command writes nothing, .trama/ included");
        });

        test("does not look in a parent folder for .weaver/", async () => {
          const project = await newProject(environment);
          const inner = makeFolder(project, "inner");
          const result = await exec(withArgs("--input", "{}"), { cwd: inner });
          assert.equal(result.status, 1, `${result.stdout}${result.stderr}`);
          assert.match(result.stderr, /initiali[sz]e/i);
        });
      }

      test("an unknown field is invalid input: exit 2 naming the field, nothing written", async () => {
        const root = await newProject(environment);
        const before = snapshot(root);
        const result = await exec(withArgs("--root", root, "--input", '{"noSuchField":1}'), { cwd: root });
        assert.equal(result.status, 2, `${result.stdout}${result.stderr}`);
        assert.ok(result.stderr.includes("noSuchField"), result.stderr);
        assert.deepEqual(snapshot(root), before);
      });

      test("with --json the error is also {error: {code, message}} on stdout", async () => {
        const root = await newProject(environment);
        const result = await exec(withArgs("--root", root, "--json", "--input", '{"noSuchField":1}'), { cwd: root });
        assert.equal(result.status, 2);
        const body = parseJson(result.stdout);
        assert.equal(typeof body?.error?.code, "string", result.stdout);
        assert.equal(typeof body?.error?.message, "string", result.stdout);
        assert.ok(result.stderr.trim().length > 0, "the error is on stderr too");
      });

      test("input that is not a JSON object is exit 2", async () => {
        const root = await newProject(environment);
        for (const bad of ["{", "[]", '"text"', "1"]) {
          const result = await exec(withArgs("--root", root, "--input", bad), { cwd: root });
          assert.equal(result.status, 2, `input ${bad}: ${result.stdout}${result.stderr}`);
        }
      });

      test("every missing required field and every wrong type is listed with its path", async () => {
        const root = await newProject(environment);
        const input = await schema();
        const properties = input.properties ?? {};
        const required = input.required ?? [];
        if (required.length > 0) {
          const result = await exec(withArgs("--root", root, "--input", "{}"), { cwd: root });
          assert.equal(result.status, 2, `${result.stdout}${result.stderr}`);
          for (const field of required) assert.ok(result.stderr.includes(field), `missing ${field} not listed: ${result.stderr}`);
        }
        const wrong = Object.entries(properties)
          .map(([field, sub]) => [field, wrongTypeFor(sub)])
          .filter(([, value]) => value !== undefined);
        for (const [field, value] of wrong) {
          const body = JSON.stringify({ [field]: value });
          const result = await exec(withArgs("--root", root, "--input", body), { cwd: root });
          assert.equal(result.status, 2, `${body}: ${result.stdout}${result.stderr}`);
          assert.ok(result.stderr.includes(field), `${field} not named: ${result.stderr}`);
        }
        if (wrong.length > 0) {
          const everything = Object.fromEntries(wrong);
          everything.noSuchField = true;
          const result = await exec(withArgs("--root", root, "--input", JSON.stringify(everything)), { cwd: root });
          assert.equal(result.status, 2);
          for (const field of [...Object.keys(everything)]) assert.ok(result.stderr.includes(field), `${field} not listed: ${result.stderr}`);
        }
      });

      test("--input - reads stdin; a leading UTF-8 BOM changes nothing", async () => {
        const root = await newProject(environment);
        const body = '{"noSuchField":1}';
        const plain = await exec(withArgs("--root", root, "--input", "-"), { cwd: root, stdin: body });
        const bom = await exec(withArgs("--root", root, "--input", "-"), { cwd: root, stdin: `\uFEFF${body}` });
        assert.equal(plain.status, 2, `${plain.stdout}${plain.stderr}`);
        assert.equal(bom.status, plain.status);
        assert.equal(bom.stderr, plain.stderr, "a BOM is not reported as a syntax error");
        assert.ok(plain.stderr.includes("noSuchField"));
      });
    });
  }
});

/**
 * A command of the registry whose handler another Task writes answers `not-implemented` (exit 1)
 * in a fresh project. Its behaviour tests are skipped, with that reason, until the Task lands;
 * the tests that need no handler (help, schemas, validation, lookups, access) always run.
 */
const pendingHandlers = new Map();
if (registryBuilt) {
  const probe = await newProject(environment);
  for (const [object, verb] of READS) {
    const result = await exec([object, verb, "--root", probe, "--json"], { cwd: probe });
    if (parseJson(result.stdout)?.error?.code === "not-implemented") {
      pendingHandlers.set(`${object} ${verb}`, `\`weaver ${object} ${verb}\` is not built yet: its Task fills the handler of the registry entry`);
    }
  }
}

describe("read commands", { skip: SKIP }, () => {
  for (const [object, verb] of READS) {
    describe(`${object} ${verb}`, { skip: pendingHandlers.get(`${object} ${verb}`) ?? false }, () => {
      test("exit 0 with Markdown on stdout and nothing on stderr", async () => {
        const root = await newProject(environment);
        const before = snapshot(root);
        const result = await exec([object, verb, "--root", root], { cwd: root });
        assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
        assert.ok(result.stdout.trim().length > 0, "the output is not empty");
        assert.equal(parseJson(result.stdout), undefined, "the default output is Markdown, not JSON");
        assert.deepEqual(snapshot(root), before, "a read writes nothing in the project");
      });

      test("--json prints one JSON object where every card has id and number", async () => {
        const root = await newProject(environment);
        const result = await exec([object, verb, "--root", root, "--json"], { cwd: root });
        assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
        const body = parseJson(result.stdout);
        assert.ok(body !== null && typeof body === "object" && !Array.isArray(body), result.stdout);
        assertCardsHaveNumbers(body);
      });

      test("{} on stdin, with or without a BOM, gives the same output as no input", async () => {
        const root = await newProject(environment);
        const none = await exec([object, verb, "--root", root, "--json"], { cwd: root });
        for (const stdin of ["{}", "\uFEFF{}", "\uFEFF{}\n"]) {
          const result = await exec([object, verb, "--root", root, "--json", "--input", "-"], { cwd: root, stdin });
          assert.equal(result.status, 0, `${JSON.stringify(stdin)}: ${result.stdout}${result.stderr}`);
          assert.equal(result.stdout, none.stdout);
        }
      });
    });
  }
});

describe("refused commands", { skip: SKIP }, () => {
  for (const [object, verb, letter] of LOOKUPS) {
    test(`${object} ${verb} of a card that does not exist: exit 1, nothing written`, async () => {
      const root = await newProject(environment);
      const before = snapshot(root);
      const names = ["aaaaaaaa", ...(letter ? [`${letter}99`, `${letter.toUpperCase()}99`] : [])];
      for (const name of names) {
        const result = await exec([object, verb, name, "--root", root], { cwd: root });
        assert.equal(result.status, 1, `${name}: ${result.stdout}${result.stderr}`);
        assert.ok(result.stderr.trim().length > 0, "the reason is on stderr");
      }
      const json = await exec([object, verb, "aaaaaaaa", "--root", root, "--json"], { cwd: root });
      assert.equal(json.status, 1);
      const body = parseJson(json.stdout);
      assert.equal(typeof body?.error?.code, "string", json.stdout);
      assert.equal(typeof body?.error?.message, "string", json.stdout);
      assert.deepEqual(snapshot(root), before);
    });
  }
});

// The test suite sets the variables it needs itself; it never reads the inherited ones.
const asJob = (extra = {}) => ({ env: { ...environment.env, WEAVER_JOB: "jjjjjjjj", ...extra } });
const asGeneral = () => ({ env: { ...environment.env, WEAVER_GENERAL: "gggggggg" } });

describe("who may run what (requirements/cli#agent)", { skip: SKIP }, () => {
  const JOB_BOUND = [
    [["task", "move", "aaaaaaaa", "doing"], "{}"],
    [["task", "move", "aaaaaaaa", "todo"], "{}"],
    [["task", "move", "aaaaaaaa", "done"], '{"unchanged":"checked"}'],
    [["announcement", "new"], '{"kind":"task","title":"Cancel an order"}'],
    [["job", "edit"], '{"note":"Reading the Machine"}'],
  ];
  for (const [words, input] of JOB_BOUND) {
    test(`${words.join(" ")} without WEAVER_JOB: exit 1, only the agent of a Weaver job can, nothing written`, async () => {
      const root = await newProject(environment);
      const before = snapshot(root);
      const result = await exec([...words, "--root", root, "--input", input], { cwd: root });
      assert.equal(result.status, 1, `${result.stdout}${result.stderr}`);
      assert.match(result.stderr, /agent of a job that Weaver started/);
      assert.deepEqual(snapshot(root), before);
      const json = await exec([...words, "--root", root, "--input", input, "--json"], { cwd: root });
      assert.equal(parseJson(json.stdout)?.error?.code, "forbidden", json.stdout);
    });

    test(`${words.join(" ")} with a WEAVER_JOB and no app: exit 1 saying Weaver is not running, nothing written`, async () => {
      const root = await newProject(environment);
      const before = snapshot(root);
      const result = await run([...words, "--root", root, "--input", input, "--json"], { ...asJob(), cwd: root });
      assert.equal(result.status, 1, `${result.stdout}${result.stderr}`);
      assert.match(result.stderr, /Weaver is not running/);
      assert.equal(parseJson(result.stdout)?.error?.code, "app-not-running");
      assert.deepEqual(snapshot(root), before);
    });
  }

  test("job new needs the identity of General: without it exit 1, with it and no app exit 1 (Weaver not running)", async () => {
    const root = await newProject(environment);
    const before = snapshot(root);
    const without = await exec(["job", "new", "--root", root, "--input", '{"kind":"scan"}'], { cwd: root });
    assert.equal(without.status, 1);
    assert.match(without.stderr, /General/);
    const asAgentOfAJob = await run(["job", "new", "--root", root, "--input", '{"kind":"scan"}'], { ...asJob(), cwd: root });
    assert.equal(asAgentOfAJob.status, 1, "a job is not General");
    assert.match(asAgentOfAJob.stderr, /General/);
    const general = await run(["job", "new", "--root", root, "--input", '{"kind":"scan"}'], { ...asGeneral(), cwd: root });
    assert.equal(general.status, 1);
    assert.match(general.stderr, /Weaver is not running/);
    assert.deepEqual(snapshot(root), before);
  });

  test("request new is for the agent of Chat only: any other caller exit 1, Chat with no app exit 1 (Weaver not running), blank text exit 2", async () => {
    const root = await newProject(environment);
    const before = snapshot(root);
    const input = '{"text":"# Cart\\n\\nAdd a cart."}';
    const without = await exec(["request", "new", "--root", root, "--input", input], { cwd: root });
    assert.equal(without.status, 1);
    assert.match(without.stderr, /Only the agent of Chat can run `weaver request new`/);
    const asAgentOfAJob = await run(["request", "new", "--root", root, "--input", input], { ...asJob(), cwd: root });
    assert.equal(asAgentOfAJob.status, 1, "a job is not Chat");
    assert.match(asAgentOfAJob.stderr, /Only the agent of Chat/);
    const chat = await run(["request", "new", "--root", root, "--input", input], { ...asGeneral(), cwd: root });
    assert.equal(chat.status, 1);
    assert.match(chat.stderr, /Weaver is not running/);
    const blank = await run(["request", "new", "--root", root, "--input", '{"text":"  "}'], { ...asGeneral(), cwd: root });
    assert.equal(blank.status, 2);
    assert.match(blank.stderr, /text/);
    assert.deepEqual(snapshot(root), before);
  });

  const QUESTION = JSON.stringify({
    questions: [
      { id: "a", question: "Which one?", options: [{ label: "x", description: "first" }, { label: "y", description: "second" }], recommended: 0, multi: false },
    ],
  });

  test("question new without WEAVER_JOB and WEAVER_GENERAL answers unavailable on stdout, exit 0, with or without --json", async () => {
    const root = await newProject(environment);
    for (const extra of [[], ["--json"]]) {
      const result = await exec(["question", "new", "--root", root, "--input", "-", ...extra], { cwd: root, stdin: QUESTION });
      assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
      const body = parseJson(result.stdout);
      assert.equal(body?.status, "unavailable", result.stdout);
      assert.match(body.reason, /Do not ask in any other way: stop and tell the developer\./);
    }
  });

  test("question new in a job with no app answers unavailable too", async () => {
    const root = await newProject(environment);
    const result = await run(["question", "new", "--root", root, "--input", "-"], { ...asJob(), cwd: root, stdin: QUESTION });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
    assert.equal(parseJson(result.stdout)?.status, "unavailable");
  });

  test("question new in the Chat with no app answers unavailable too", async () => {
    const root = await newProject(environment);
    const result = await run(["question", "new", "--root", root, "--input", "-"], { ...asGeneral(), cwd: root, stdin: QUESTION });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
    assert.equal(parseJson(result.stdout)?.status, "unavailable");
  });

  test("question new --help names the Chat: during a turn, and only when the developer asked for a round", async () => {
    const result = await exec(["question", "new", "--help"], { cwd: emptyFolder() });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /in Chat, use it only when the developer asked to be asked with a round/i);
    assert.match(result.stdout, /neither in a working Weaver job nor in the project's Chat during a turn/);
    assert.match(result.stdout, /agent of the project's Chat during a turn can use it/);
  });

  test("question new validates the whole round first: exit 2 with the paths, JSON on stdout without --json", async () => {
    const root = await newProject(environment);
    const bad = JSON.stringify({ questions: [{ id: "a", question: "Q", options: [{ label: "x", description: "d" }, { label: "", description: "d" }], recommended: 5, multi: false }] });
    // An endpoint with no app behind it: the round is validated before the app is contacted.
    const result = await run(["question", "new", "--root", root, "--input", "-"], { ...asJob({ WEAVER_ENDPOINT: "/nonexistent" }), cwd: root, stdin: bad });
    assert.equal(result.status, 2, `${result.stdout}${result.stderr}`);
    assert.ok(result.stderr.includes("questions[0].options[1].label"), result.stderr);
    assert.ok(result.stderr.includes("questions[0].recommended"), result.stderr);
    const body = parseJson(result.stdout);
    assert.equal(body?.error?.code, "invalid-input", result.stdout);
  });

  test("decision commands accept any caller: they are refused for the card, not for the caller", async () => {
    const root = await newProject(environment);
    for (const words of [["proposal", "move", "p1", "discarded"], ["drift", "move", "d1", "dismissed"], ["task", "delete", "t1"]]) {
      const result = await exec([...words, "--root", root], { cwd: root });
      assert.equal(result.status, 1, `${words.join(" ")}: ${result.stdout}${result.stderr}`);
      assert.doesNotMatch(result.stderr, /agent of a job/);
      assert.match(result.stderr, /not the number of any live/);
    }
  });

  test("proposal new, edit and move open are refused whenever WEAVER_GENERAL is set, without asking the app", async () => {
    const root = await newProject(environment);
    for (const [words, input] of [
      [["proposal", "new"], '{"title":"Cancel an order"}'],
      [["proposal", "edit", "aaaaaaaa"], "{}"],
      [["proposal", "move", "aaaaaaaa", "open"], "{}"],
    ]) {
      const result = await run([...words, "--root", root, "--json", "--input", input], { ...asGeneral(), cwd: root });
      assert.equal(result.status, 1, `${words.join(" ")}: ${result.stdout}${result.stderr}`);
      assert.match(parseJson(result.stdout)?.error?.message ?? "", /Chat does not write Proposals.*weaver request new/s);
    }
    assert.equal(existsSync(path.join(root, ".weaver", "proposals")), false, "nothing was written");
  });

  test("proposal new from a WEAVER_JOB no app vouches for files a plain Draft", async () => {
    const root = await newProject(environment);
    const result = await run(["proposal", "new", "--root", root, "--json", "--input", '{"title":"Cancel an order"}'], { ...asJob(), cwd: root });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
    const body = parseJson(result.stdout);
    const folder = readdirSync(path.join(root, ".weaver", "proposals")).find((name) => name.startsWith(body.id));
    const text = readFileSync(path.join(root, ".weaver", "proposals", folder, "proposal.md"), "utf8");
    assert.doesNotMatch(text, /^(chat(Writing)?|job|request):/m, "an unrecognised identity counts as none");
  });

  test("the help of the proposal commands says what the caller binds and never talks about Chat", async () => {
    for (const verb of ["new", "show", "list", "edit", "move"]) {
      const help = await exec(["proposal", verb, "--help"], { cwd: emptyFolder() });
      assert.equal(help.status, 0, help.stderr);
      assert.doesNotMatch(help.stdout, /Chat/, `proposal ${verb}`);
    }
    const created = await exec(["proposal", "new", "--help"], { cwd: emptyFolder() });
    assert.match(created.stdout, /any other caller binds nothing/);
    const moved = await exec(["proposal", "move", "--help"], { cwd: emptyFolder() });
    assert.match(moved.stdout, /any caller can run them under the same conditions/);
  });

  test("a Task number for a Proposal, and a number no live card has, are refused with exit 1", async () => {
    const root = await newProject(environment);
    const wrongLetter = await exec(["proposal", "show", "t3", "--root", root], { cwd: root });
    assert.equal(wrongLetter.status, 1);
    assert.match(wrongLetter.stderr, /Task number/);
    const notLive = await exec(["task", "show", "t3", "--root", root], { cwd: root });
    assert.equal(notLive.status, 1);
    assert.match(notLive.stderr, /not the number of any live Task/);
  });

  test("commands that ask the app fail with exit 1 when it is closed", async () => {
    const root = await newProject(environment);
    for (const words of [["job", "list"], ["job", "show", "aaaaaaaa"]]) {
      const result = await exec([...words, "--root", root, "--json"], { cwd: root });
      assert.equal(result.status, 1, `${words.join(" ")}: ${result.stdout}${result.stderr}`);
      assert.equal(parseJson(result.stdout)?.error?.code, "app-not-running");
    }
  });

  test("job show without an id says it is outside a job, exit 0", async () => {
    const root = await newProject(environment);
    const result = await exec(["job", "show", "--root", root, "--json"], { cwd: root });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
    assert.deepEqual(parseJson(result.stdout), { job: null });
  });

  test("the WEAVER_* variables change nothing for a command that reads", async () => {
    const root = await newProject(environment);
    const clean = await exec(["model", "check", "--root", root, "--json"], { cwd: root });
    const polluted = await run(["model", "check", "--root", root, "--json"], {
      cwd: root,
      env: { ...environment.env, WEAVER_JOB: "jjjjjjjj", WEAVER_GENERAL: "gggggggg", WEAVER_ENDPOINT: "/nonexistent", WEAVER_RUN: "1", WEAVER_TASK: "tttttttt" },
    });
    assert.equal(polluted.status, clean.status);
    assert.equal(polluted.stdout, clean.stdout);
  });
});

describe("model commands", { skip: SKIP }, () => {
  const write = (root, rel, text) => {
    const file = path.join(root, ".weaver", rel);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, text);
  };
  const MACHINE = "# Order\n\nAn order.\n\n## States\n\n### Draft (initial)\nChoosing.\n- `user.CANCEL` → Cancelled: refunds.\n\n### Cancelled (final)\n";
  const modelProject = async () => {
    const root = await newProject(environment);
    write(root, "model/checkout/feature.md", "# Checkout\n\nBuying things.\n");
    write(root, "model/checkout/order.md", MACHINE);
    write(root, "model/actors/user.md", "# User\n\nA shopper.\n\n## Events\n\n### CANCEL\nThe shopper cancels.\n");
    write(root, "requirements/ui/checkout.md", "---\ndescription: Use when building checkout.\n---\n# Checkout\n\n## General\n- Material 3.\n\n## order/Draft\n- A grey badge.\n");
    return root;
  };

  test("model show without a reference prints the System; with a reference it prints the element and its Requirements", async () => {
    const root = await modelProject();
    const system = await exec(["model", "show", "--root", root], { cwd: root });
    assert.equal(system.status, 0, system.stderr);
    assert.match(system.stdout, /^# System: /);
    assert.match(system.stdout, /`checkout`: Checkout\. Machines: `order`/);
    const state = await exec(["model", "show", "order/Draft", "--root", root], { cwd: root });
    assert.equal(state.status, 0, state.stderr);
    assert.match(state.stdout, /^# State `order\/Draft`/);
    assert.match(state.stdout, /A grey badge/);
    const general = await exec(["model", "show", "requirements/ui#General", "--root", root], { cwd: root });
    assert.match(general.stdout, /Material 3/);
    const json = await exec(["model", "show", "order", "--root", root, "--json"], { cwd: root });
    const body = parseJson(json.stdout);
    assert.equal(body.kind, "machine");
    assertCardsHaveNumbers(body);
  });

  test("model show of what does not exist is refused with exit 1", async () => {
    const root = await modelProject();
    for (const ref of ["nothing", "order/Nowhere", "requirements/none", "Bad Ref"]) {
      const result = await exec(["model", "show", ref, "--root", root], { cwd: root });
      assert.equal(result.status, 1, `${ref}: ${result.stdout}${result.stderr}`);
    }
  });

  test("model list {text} searches the Model and the Requirements", async () => {
    const root = await modelProject();
    const result = await exec(["model", "list", "--root", root, "--input", '{"text":"GREY"}'], { cwd: root });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /`requirements\/ui#order\/Draft` \(section\)/);
    const none = await exec(["model", "list", "--root", root, "--input", '{"text":"zzzzzz"}'], { cwd: root });
    assert.match(none.stdout, /Nothing found\./);
    const blank = await exec(["model", "list", "--root", root, "--input", '{"text":"  "}'], { cwd: root });
    assert.equal(blank.status, 2);
  });

  test("model check exits 1 on an error and 0 on a warning only", async () => {
    const root = await modelProject();
    write(root, "requirements/ui/big.md", `---\ndescription: Use when big.\n---\n# Big\n\n## user\n${"- a line of text\n".repeat(1000)}`);
    const warning = await exec(["model", "check", "--root", root, "--json"], { cwd: root });
    assert.equal(warning.status, 0, warning.stdout);
    assert.equal(parseJson(warning.stdout).warnings, 1);
    write(root, "model/checkout/order.md", "# Order\n\n## States\n\n### Draft\n- `user.NOPE` → Gone\n");
    const error = await exec(["model", "check", "--root", root], { cwd: root });
    assert.equal(error.status, 1);
    assert.match(error.stdout, /model\/checkout\/order\.md/);
    assert.match(error.stdout, /error/);
  });
});

describe("task commands", { skip: SKIP || pendingHandlers.get("task list") || false }, () => {
  const fileTask = async (root, input) => {
    const result = await exec(["task", "new", "--root", root, "--json", "--input", JSON.stringify(input)], { cwd: root });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
    return parseJson(result.stdout);
  };

  test("task new files a Task in To do; show, list and delete work on it by identity and by number", async () => {
    const root = await newProject(environment);
    const created = await fileTask(root, { title: "Tidy the logs", technical: true, body: "Logs name no secret." });
    assert.match(created.id, /^[a-z0-9]{8}$/);
    assert.equal(created.column, "todo");
    assert.ok(Number.isInteger(created.number));
    const files = entries(path.join(root, ".weaver", "tasks"));
    assert.deepEqual(files, [`${created.id}-tidy-the-logs.md`]);

    const shown = await exec(["task", "show", `t${created.number}`, "--root", root, "--json"], { cwd: root });
    assert.equal(shown.status, 0, `${shown.stdout}${shown.stderr}`);
    const body = parseJson(shown.stdout);
    assert.equal(body.id, created.id);
    assert.equal(body.technical, true);
    assertCardsHaveNumbers(body);

    const listed = await exec(["task", "list", "--root", root, "--json", "--input", '{"column":"todo"}'], { cwd: root });
    assert.equal(parseJson(listed.stdout).tasks.length, 1);
    const runnable = await exec(["task", "list", "--root", root, "--json", "--input", '{"runnable":true}'], { cwd: root });
    assert.equal(parseJson(runnable.stdout).tasks[0].id, created.id);

    const deleted = await exec(["task", "delete", created.id, "--root", root], { cwd: root });
    assert.equal(deleted.status, 0, `${deleted.stdout}${deleted.stderr}`);
    assert.deepEqual(entries(path.join(root, ".weaver", "tasks")), []);
  });

  test("task new: a technical Task with references and a non-technical one without are refused, nothing written", async () => {
    const root = await newProject(environment);
    const before = snapshot(root);
    for (const input of [{ title: "A", technical: true, refs: ["order"] }, { title: "B" }, { title: "C", refs: ["nothing"] }]) {
      const result = await exec(["task", "new", "--root", root, "--input", JSON.stringify(input)], { cwd: root });
      assert.equal(result.status, 1, `${JSON.stringify(input)}: ${result.stdout}${result.stderr}`);
    }
    assert.deepEqual(snapshot(root), before);
  });

  test("task list: max, groups and heldBack without runnable are exit 2", async () => {
    const root = await newProject(environment);
    for (const input of ['{"max":2}', '{"groups":true}', '{"heldBack":true}']) {
      const result = await exec(["task", "list", "--root", root, "--input", input], { cwd: root });
      assert.equal(result.status, 2, `${input}: ${result.stdout}${result.stderr}`);
    }
  });
});

describe("project new", { skip: SKIP }, () => {
  test("creates .weaver/ in a fresh folder: exit 0, Markdown on stdout", async () => {
    const root = emptyFolder();
    const result = await exec(["project", "new", "--input", "{}", "--root", root], { cwd: root });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
    assert.ok(existsSync(path.join(root, ".weaver")));
    assert.ok(result.stdout.trim().length > 0);
  });

  test("--json prints one JSON object", async () => {
    const root = emptyFolder();
    const result = await exec(["project", "new", "--input", "{}", "--root", root, "--json"], { cwd: root });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
    assert.equal(typeof parseJson(result.stdout), "object");
  });

  test("--input - accepts {} with and without a UTF-8 BOM", async () => {
    for (const stdin of ["{}", "\uFEFF{}"]) {
      const root = emptyFolder();
      const result = await exec(["project", "new", "--input", "-", "--root", root], { cwd: root, stdin });
      assert.equal(result.status, 0, `${JSON.stringify(stdin)}: ${result.stdout}${result.stderr}`);
      assert.ok(existsSync(path.join(root, ".weaver")));
    }
  });

  test("an unknown field is exit 2 and creates no .weaver/", async () => {
    const root = emptyFolder();
    const result = await exec(["project", "new", "--input", '{"noSuchField":1}', "--root", root], { cwd: root });
    assert.equal(result.status, 2, `${result.stdout}${result.stderr}`);
    assert.ok(result.stderr.includes("noSuchField"));
    assert.deepEqual(entries(root), []);
  });

  test("beside an old .trama/ it creates .weaver/ and leaves .trama/ as it was", async () => {
    const root = emptyFolder();
    makeFolder(root, ".trama/model");
    writeFileSync(path.join(root, ".trama", "model", "system.md"), "# Old\n");
    const result = await exec(["project", "new", "--input", "{}", "--root", root], { cwd: root });
    assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
    assert.ok(existsSync(path.join(root, ".weaver", "model", "system.md")));
    assert.equal(readFileSync(path.join(root, ".trama", "model", "system.md"), "utf8"), "# Old\n");
  });

  test("a second run is refused with exit 1 and changes nothing", async () => {
    const root = await newProject(environment);
    const before = snapshot(root);
    const result = await exec(["project", "new", "--input", "{}", "--root", root], { cwd: root });
    assert.equal(result.status, 1, `${result.stdout}${result.stderr}`);
    assert.match(result.stderr, /already exists/);
    assert.deepEqual(snapshot(root), before);
  });

  test("a fresh project passes `model check`, has the five Areas and installs the agent files", async () => {
    const root = await newProject(environment);
    assert.deepEqual(entries(path.join(root, ".weaver", "requirements")), ["data", "quality", "rules", "tech", "ui"]);
    assert.ok(existsSync(path.join(root, ".agents", "skills", "weaver-design", "COMMANDS.md")));
    assert.ok(existsSync(path.join(root, ".omp", "WATCHDOG.yml")));
    const check = await exec(["model", "check", "--root", root], { cwd: root });
    assert.equal(check.status, 0, `${check.stdout}${check.stderr}`);
  });

  test("agent files that differ are all listed and nothing is created, unless force is true", async () => {
    const root = emptyFolder();
    for (const file of [".agents/skills/weaver-design/SKILL.md", ".omp/WATCHDOG.yml"]) {
      makeFolder(root, path.dirname(file));
      writeFileSync(path.join(root, file), "mine");
    }
    const before = snapshot(root);
    const refused = await exec(["project", "new", "--input", "{}", "--root", root], { cwd: root });
    assert.equal(refused.status, 1, `${refused.stdout}${refused.stderr}`);
    assert.ok(refused.stderr.includes(".agents/skills/weaver-design/SKILL.md") && refused.stderr.includes(".omp/WATCHDOG.yml"), refused.stderr);
    assert.deepEqual(snapshot(root), before);
    const forced = await exec(["project", "new", "--input", '{"force":true}', "--root", root], { cwd: root });
    assert.equal(forced.status, 0, `${forced.stdout}${forced.stderr}`);
    assert.ok(existsSync(path.join(root, ".weaver")));
    assert.notEqual(readFileSync(path.join(root, ".omp", "WATCHDOG.yml"), "utf8"), "mine");
  });
});

describe("the solo field of project new, project edit and project show", { skip: SKIP }, () => {
  const excludeOf = (root) => readFileSync(path.join(root, ".git", "info", "exclude"), "utf8");

  test("--help of project new and project edit names the field and its three values", async () => {
    for (const verb of ["new", "edit"]) {
      const result = await exec(["project", verb, "--help"], { cwd: emptyFolder() });
      assert.equal(result.status, 0, result.stderr);
      for (const word of ["solo", "off", "folders", "files"]) assert.ok(result.stdout.includes(word), `${verb}: ${word}\n${result.stdout}`);
      const json = await exec(["project", verb, "--help", "--json"], { cwd: emptyFolder() });
      const schema = parseJson(json.stdout);
      assert.deepEqual(schema.input.properties.solo.enum, ["off", "folders", "files"], `${verb}: input`);
      assert.ok(schema.output.properties.solo, `${verb}: output`);
    }
    const show = parseJson((await exec(["project", "show", "--help", "--json"], { cwd: emptyFolder() })).stdout);
    assert.ok(show.output.properties.solo, "project show: output");
  });

  test("another value is exit 2 with the field path, and nothing is created or changed", async () => {
    const root = gitRepository();
    const created = await exec(["project", "new", "--input", '{"solo":"all"}', "--root", root], { cwd: root });
    assert.equal(created.status, 2, `${created.stdout}${created.stderr}`);
    assert.ok(created.stderr.includes("solo"), created.stderr);
    assert.ok(!existsSync(path.join(root, ".weaver")));

    const project = await newProject(environment);
    const before = snapshot(project);
    for (const input of ['{"solo":"all"}', '{"solo":true}', '{"solo":"Folders"}']) {
      const result = await exec(["project", "edit", "--input", input, "--root", project], { cwd: project });
      assert.equal(result.status, 2, `${input}: ${result.stdout}${result.stderr}`);
      assert.ok(result.stderr.includes("solo"), result.stderr);
    }
    assert.deepEqual(snapshot(project), before);
  });

  for (const [mode, line] of [
    ["folders", "/.omp/"],
    ["files", "/.omp/WATCHDOG.yml"],
  ]) {
    test(`project new with solo ${mode} in a repository: exit 0, the block is written, Markdown and --json say so`, async () => {
      const root = gitRepository();
      const result = await exec(["project", "new", "--input", JSON.stringify({ solo: mode }), "--root", root, "--json"], { cwd: root });
      assert.equal(result.status, 0, `${result.stdout}${result.stderr}`);
      assert.deepEqual(parseJson(result.stdout).solo, { mode, tracked: [] });
      assert.ok(excludeOf(root).includes(`\n${line}\n`), excludeOf(root));
      assert.equal(readFileSync(path.join(root, ".weaver", "solo", "solo.json"), "utf8"), `{"mode": "${mode}"}\n`);
      assert.equal(git(root, ["status", "--porcelain"]), "", "git sees only the code");

      const other = gitRepository();
      const text = await exec(["project", "new", "--input", JSON.stringify({ solo: mode }), "--root", other], { cwd: other });
      assert.equal(text.status, 0, `${text.stdout}${text.stderr}`);
      assert.match(text.stdout, /## Solo mode/);
    });
  }

  test("project new with solo off, or with no solo, leaves git alone and prints solo off", async () => {
    for (const input of ["{}", '{"solo":"off"}']) {
      const root = gitRepository();
      const before = excludeOf(root);
      const result = await exec(["project", "new", "--input", input, "--root", root, "--json"], { cwd: root });
      assert.equal(result.status, 0, `${input}: ${result.stdout}${result.stderr}`);
      assert.deepEqual(parseJson(result.stdout).solo, { mode: "off", tracked: [] });
      assert.equal(excludeOf(root), before);
      assert.ok(!existsSync(path.join(root, ".weaver", "solo")));
    }
  });

  test("project new with solo outside a git repository is exit 1 with the reason and creates nothing", async () => {
    const root = emptyFolder();
    const result = await exec(["project", "new", "--input", '{"solo":"folders"}', "--root", root], { cwd: root });
    assert.equal(result.status, 1, `${result.stdout}${result.stderr}`);
    assert.match(result.stderr, /this folder is not in a git repository/);
    assert.deepEqual(entries(root), []);
  });

  test("project edit with solo changes only Solo mode and prints it; project show prints the same", async () => {
    const root = gitRepository();
    const created = await exec(["project", "new", "--input", "{}", "--root", root], { cwd: root });
    assert.equal(created.status, 0, `${created.stdout}${created.stderr}`);
    git(root, ["add", "-f", ".weaver/model/system.md"]);
    const skills = readFileSync(path.join(root, ".agents", "skills", "weaver-design", "SKILL.md"), "utf8");

    const on = await exec(["project", "edit", "--input", '{"solo":"files"}', "--root", root, "--json"], { cwd: root });
    assert.equal(on.status, 0, `${on.stdout}${on.stderr}`);
    assert.deepEqual(parseJson(on.stdout), { solo: { mode: "files", tracked: [".weaver/model/system.md"] } });
    assert.ok(excludeOf(root).includes("\n/.omp/WATCHDOG.yml\n"));
    assert.equal(readFileSync(path.join(root, ".agents", "skills", "weaver-design", "SKILL.md"), "utf8"), skills);

    const text = await exec(["project", "edit", "--input", '{"solo":"files"}', "--root", root], { cwd: root });
    assert.equal(text.status, 0, `${text.stdout}${text.stderr}`);
    assert.match(text.stdout, /Git still tracks 1 of Weaver's files/);
    assert.ok(text.stdout.includes(".weaver/model/system.md"));

    const shown = await exec(["project", "show", "--root", root, "--json"], { cwd: root });
    assert.equal(shown.status, 0, `${shown.stdout}${shown.stderr}`);
    assert.deepEqual(parseJson(shown.stdout).solo, { mode: "files", tracked: [".weaver/model/system.md"] });

    const folders = await exec(["project", "edit", "--input", '{"solo":"folders"}', "--root", root, "--json"], { cwd: root });
    assert.equal(parseJson(folders.stdout).solo.mode, "folders");
    const off = await exec(["project", "edit", "--input", '{"solo":"off"}', "--root", root, "--json"], { cwd: root });
    assert.equal(off.status, 0, `${off.stdout}${off.stderr}`);
    assert.deepEqual(parseJson(off.stdout), { solo: { mode: "off", tracked: [] } });
    assert.ok(!excludeOf(root).includes("weaver solo mode"));
    assert.ok(!existsSync(path.join(root, ".weaver", "solo")));
  });

  test("project edit with solo that cannot change is exit 1 with the reason and changes nothing", async () => {
    const root = await newProject(environment);
    const before = snapshot(root);
    const result = await exec(["project", "edit", "--input", '{"solo":"folders"}', "--root", root], { cwd: root });
    assert.equal(result.status, 1, `${result.stdout}${result.stderr}`);
    assert.match(result.stderr, /Could not change Solo mode: this folder is not in a git repository/);
    assert.deepEqual(snapshot(root), before);
  });

  test("project show says why Solo mode cannot be changed, and why its block is not in place, and repairs nothing", async () => {
    const plain = await newProject(environment);
    const shown = parseJson((await exec(["project", "show", "--root", plain, "--json"], { cwd: plain })).stdout);
    assert.deepEqual(shown.solo, { mode: "off", unavailable: "this folder is not in a git repository", tracked: [] });

    const root = gitRepository();
    await exec(["project", "new", "--input", '{"solo":"folders"}', "--root", root], { cwd: root });
    const exclude = path.join(root, ".git", "info", "exclude");
    writeFileSync(exclude, excludeOf(root).replace("# >>> weaver solo mode", "# nothing"));
    const broken = excludeOf(root);
    const result = await exec(["project", "show", "--root", root, "--json"], { cwd: root });
    const solo = parseJson(result.stdout).solo;
    assert.equal(solo.mode, "folders");
    assert.equal(typeof solo.problem, "string");
    assert.equal(solo.unavailable, undefined);
    assert.equal(excludeOf(root), broken);
  });
});

test("the binary under test is the one this run located", () => {
  assert.ok(existsSync(WEAVER_CLI), WEAVER_CLI);
});
