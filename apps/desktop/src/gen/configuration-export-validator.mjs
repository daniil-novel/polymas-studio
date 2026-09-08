// Generated from contracts/configuration-export.schema.json; run npm run generate:contracts.
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function __require() {
  try {
    return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
  } catch (e) {
    throw mod = 0, e;
  }
};

// node_modules/fast-deep-equal/index.js
var require_fast_deep_equal = __commonJS({
  "node_modules/fast-deep-equal/index.js"(exports, module) {
    "use strict";
    module.exports = function equal(a, b) {
      if (a === b) return true;
      if (a && b && typeof a == "object" && typeof b == "object") {
        if (a.constructor !== b.constructor) return false;
        var length, i, keys;
        if (Array.isArray(a)) {
          length = a.length;
          if (length != b.length) return false;
          for (i = length; i-- !== 0; )
            if (!equal(a[i], b[i])) return false;
          return true;
        }
        if (a.constructor === RegExp) return a.source === b.source && a.flags === b.flags;
        if (a.valueOf !== Object.prototype.valueOf) return a.valueOf() === b.valueOf();
        if (a.toString !== Object.prototype.toString) return a.toString() === b.toString();
        keys = Object.keys(a);
        length = keys.length;
        if (length !== Object.keys(b).length) return false;
        for (i = length; i-- !== 0; )
          if (!Object.prototype.hasOwnProperty.call(b, keys[i])) return false;
        for (i = length; i-- !== 0; ) {
          var key = keys[i];
          if (!equal(a[key], b[key])) return false;
        }
        return true;
      }
      return a !== a && b !== b;
    };
  }
});

// node_modules/ajv/dist/runtime/equal.js
var require_equal = __commonJS({
  "node_modules/ajv/dist/runtime/equal.js"(exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    var equal = require_fast_deep_equal();
    equal.code = 'require("ajv/dist/runtime/equal").default';
    exports.default = equal;
  }
});

// node_modules/ajv/dist/runtime/ucs2length.js
var require_ucs2length = __commonJS({
  "node_modules/ajv/dist/runtime/ucs2length.js"(exports) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    function ucs2length(str) {
      const len = str.length;
      let length = 0;
      let pos = 0;
      let value;
      while (pos < len) {
        length++;
        value = str.charCodeAt(pos++);
        if (value >= 55296 && value <= 56319 && pos < len) {
          value = str.charCodeAt(pos);
          if ((value & 64512) === 56320)
            pos++;
        }
      }
      return length;
    }
    exports.default = ucs2length;
    ucs2length.code = 'require("ajv/dist/runtime/ucs2length").default';
  }
});

// configuration-export-validator.cjs
var require_configuration_export_validator = __commonJS({
  "configuration-export-validator.cjs"(exports, module) {
    module.exports = validate20;
    module.exports.default = validate20;
    var schema31 = { "$schema": "https://json-schema.org/draft/2020-12/schema", "$id": "https://polymas.local/schemas/configuration-export-0.1.0.json", "title": "PolyMAS materialized configuration export", "type": "object", "additionalProperties": false, "properties": { "format": { "const": "polymas-configuration" }, "schemaVersion": { "const": "0.1.0" }, "migrationVersion": { "const": 1 }, "sourceRevision": { "type": "integer", "minimum": 1 }, "name": { "type": "string" }, "runtime": { "type": "object", "additionalProperties": false, "properties": { "adapter": { "const": "codex-exec" }, "setupRequired": { "const": true } }, "required": ["adapter", "setupRequired"] }, "roleVersions": { "type": "array", "items": { "$ref": "#/$defs/role" }, "maxItems": 1e5 }, "instances": { "type": "array", "items": { "$ref": "#/$defs/instance" }, "maxItems": 1e5 }, "architecture": { "$ref": "#/$defs/architecture" }, "connections": { "type": "array", "items": { "$ref": "#/$defs/connection" }, "maxItems": 1e3 }, "bindings": { "type": "array", "items": { "$ref": "#/$defs/binding" } } }, "required": ["format", "schemaVersion", "migrationVersion", "sourceRevision", "name", "runtime", "roleVersions", "instances", "architecture", "connections", "bindings"], "$defs": { "role": { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "version": { "type": "integer", "minimum": 1 }, "name": { "type": "string" }, "description": { "type": "string" }, "instruction": { "type": "string" }, "outputContract": { "type": "string" }, "acceptanceCriteria": { "type": "string" }, "escalationRule": { "type": "string" }, "model": { "type": ["string", "null"] }, "sandbox": { "enum": ["read-only", "workspace-write"] }, "timeoutSeconds": { "type": "integer", "minimum": 5, "maximum": 86400 }, "maxTurns": { "type": "integer", "minimum": 1, "maximum": 1e3 }, "maxOutputTokens": { "type": ["integer", "null"], "minimum": 0 } }, "required": ["id", "version", "name", "description", "instruction", "outputContract", "acceptanceCriteria", "escalationRule", "model", "sandbox", "timeoutSeconds", "maxTurns", "maxOutputTokens"] }, "instance": { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "roleId": { "type": "string" }, "roleVersion": { "type": "integer", "minimum": 1 }, "archived": { "type": "boolean" } }, "required": ["id", "name", "roleId", "roleVersion", "archived"] }, "holon": { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "goal": { "type": "string" }, "parentId": { "type": ["string", "null"] }, "members": { "type": "array", "items": { "type": "string" }, "maxItems": 5e3, "uniqueItems": true }, "coordinatorAgentId": { "type": ["string", "null"] }, "inputContract": { "type": "string" }, "outputContract": { "type": "string" }, "maxTasks": { "type": "integer", "minimum": 0 } }, "required": ["id", "name", "goal", "parentId", "members", "coordinatorAgentId", "inputContract", "outputContract", "maxTasks"] }, "architecture": { "type": "object", "additionalProperties": false, "properties": { "preset": { "enum": ["single", "independent", "network", "star", "holonic", "custom"] }, "maxConcurrency": { "type": "integer", "minimum": 1, "maximum": 5e3 }, "admission": { "enum": ["queue_allowed", "start_all"] }, "coordinatorAgentId": { "type": ["string", "null"] }, "memoryEnabled": { "type": "boolean" }, "memoryConnectionId": { "type": ["string", "null"] }, "holons": { "type": "array", "items": { "$ref": "#/$defs/holon" }, "maxItems": 5e3 }, "edges": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": { "from": { "type": "string" }, "to": { "type": "string" } }, "required": ["from", "to"] }, "maxItems": 25e3 }, "positions": { "type": "object", "additionalProperties": { "type": "array", "minItems": 2, "maxItems": 2, "items": { "type": "number", "minimum": -999999, "maximum": 999999 } } }, "messageHopLimit": { "type": "integer", "minimum": 1, "maximum": 32 }, "communicationMode": { "enum": ["direct", "blackboard", "wiki", "graph"], "default": "direct" }, "maxMessagesPerRun": { "type": "integer", "minimum": 1, "maximum": 1e5 }, "paused": { "const": true } }, "required": ["preset", "maxConcurrency", "admission", "coordinatorAgentId", "memoryEnabled", "memoryConnectionId", "holons", "edges", "positions", "messageHopLimit", "maxMessagesPerRun", "paused"] }, "connection": { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "recipe": { "type": "string" }, "transport": { "enum": ["files", "stdio", "http"] }, "readOnly": { "type": "boolean" }, "timeoutSeconds": { "type": "integer", "minimum": 1, "maximum": 300 }, "toolAllowlist": { "type": "array", "items": { "type": "string" } }, "desiredEnabled": { "type": "boolean" }, "setupRequired": { "const": true }, "setupFields": { "type": "array", "items": { "enum": ["roots", "url", "command", "args", "authentication", "environment", "secret_environment"] }, "maxItems": 7, "uniqueItems": true, "minItems": 1 }, "rootCount": { "type": "integer", "minimum": 0, "maximum": 32 } }, "required": ["id", "name", "recipe", "transport", "readOnly", "timeoutSeconds", "toolAllowlist", "desiredEnabled", "setupRequired", "setupFields", "rootCount"] }, "binding": { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "connectionId": { "type": "string" }, "subject": { "type": "object", "additionalProperties": false, "properties": { "type": { "enum": ["agent", "role"] }, "id": { "type": "string" } }, "required": ["type", "id"] }, "tools": { "type": "array", "items": { "type": "string" }, "maxItems": 1e3, "minItems": 1, "uniqueItems": true }, "allowWrite": { "type": "boolean" }, "required": { "type": "boolean" }, "namespace": { "type": "string", "minLength": 1, "maxLength": 120 } }, "required": ["id", "connectionId", "subject", "tools", "allowWrite", "required", "namespace"] } } };
    var schema32 = { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "version": { "type": "integer", "minimum": 1 }, "name": { "type": "string" }, "description": { "type": "string" }, "instruction": { "type": "string" }, "outputContract": { "type": "string" }, "acceptanceCriteria": { "type": "string" }, "escalationRule": { "type": "string" }, "model": { "type": ["string", "null"] }, "sandbox": { "enum": ["read-only", "workspace-write"] }, "timeoutSeconds": { "type": "integer", "minimum": 5, "maximum": 86400 }, "maxTurns": { "type": "integer", "minimum": 1, "maximum": 1e3 }, "maxOutputTokens": { "type": ["integer", "null"], "minimum": 0 } }, "required": ["id", "version", "name", "description", "instruction", "outputContract", "acceptanceCriteria", "escalationRule", "model", "sandbox", "timeoutSeconds", "maxTurns", "maxOutputTokens"] };
    var schema36 = { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "recipe": { "type": "string" }, "transport": { "enum": ["files", "stdio", "http"] }, "readOnly": { "type": "boolean" }, "timeoutSeconds": { "type": "integer", "minimum": 1, "maximum": 300 }, "toolAllowlist": { "type": "array", "items": { "type": "string" } }, "desiredEnabled": { "type": "boolean" }, "setupRequired": { "const": true }, "setupFields": { "type": "array", "items": { "enum": ["roots", "url", "command", "args", "authentication", "environment", "secret_environment"] }, "maxItems": 7, "uniqueItems": true, "minItems": 1 }, "rootCount": { "type": "integer", "minimum": 0, "maximum": 32 } }, "required": ["id", "name", "recipe", "transport", "readOnly", "timeoutSeconds", "toolAllowlist", "desiredEnabled", "setupRequired", "setupFields", "rootCount"] };
    var schema37 = { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "connectionId": { "type": "string" }, "subject": { "type": "object", "additionalProperties": false, "properties": { "type": { "enum": ["agent", "role"] }, "id": { "type": "string" } }, "required": ["type", "id"] }, "tools": { "type": "array", "items": { "type": "string" }, "maxItems": 1e3, "minItems": 1, "uniqueItems": true }, "allowWrite": { "type": "boolean" }, "required": { "type": "boolean" }, "namespace": { "type": "string", "minLength": 1, "maxLength": 120 } }, "required": ["id", "connectionId", "subject", "tools", "allowWrite", "required", "namespace"] };
    var func1 = Object.prototype.hasOwnProperty;
    var func0 = require_equal().default;
    var func7 = require_ucs2length().default;
    var schema34 = { "type": "object", "additionalProperties": false, "properties": { "preset": { "enum": ["single", "independent", "network", "star", "holonic", "custom"] }, "maxConcurrency": { "type": "integer", "minimum": 1, "maximum": 5e3 }, "admission": { "enum": ["queue_allowed", "start_all"] }, "coordinatorAgentId": { "type": ["string", "null"] }, "memoryEnabled": { "type": "boolean" }, "memoryConnectionId": { "type": ["string", "null"] }, "holons": { "type": "array", "items": { "$ref": "#/$defs/holon" }, "maxItems": 5e3 }, "edges": { "type": "array", "items": { "type": "object", "additionalProperties": false, "properties": { "from": { "type": "string" }, "to": { "type": "string" } }, "required": ["from", "to"] }, "maxItems": 25e3 }, "positions": { "type": "object", "additionalProperties": { "type": "array", "minItems": 2, "maxItems": 2, "items": { "type": "number", "minimum": -999999, "maximum": 999999 } } }, "messageHopLimit": { "type": "integer", "minimum": 1, "maximum": 32 }, "communicationMode": { "enum": ["direct", "blackboard", "wiki", "graph"], "default": "direct" }, "maxMessagesPerRun": { "type": "integer", "minimum": 1, "maximum": 1e5 }, "paused": { "const": true } }, "required": ["preset", "maxConcurrency", "admission", "coordinatorAgentId", "memoryEnabled", "memoryConnectionId", "holons", "edges", "positions", "messageHopLimit", "maxMessagesPerRun", "paused"] };
    var schema35 = { "type": "object", "additionalProperties": false, "properties": { "id": { "type": "string" }, "name": { "type": "string" }, "goal": { "type": "string" }, "parentId": { "type": ["string", "null"] }, "members": { "type": "array", "items": { "type": "string" }, "maxItems": 5e3, "uniqueItems": true }, "coordinatorAgentId": { "type": ["string", "null"] }, "inputContract": { "type": "string" }, "outputContract": { "type": "string" }, "maxTasks": { "type": "integer", "minimum": 0 } }, "required": ["id", "name", "goal", "parentId", "members", "coordinatorAgentId", "inputContract", "outputContract", "maxTasks"] };
    function validate21(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
      let vErrors = null;
      let errors = 0;
      const evaluated0 = validate21.evaluated;
      if (evaluated0.dynamicProps) {
        evaluated0.props = void 0;
      }
      if (evaluated0.dynamicItems) {
        evaluated0.items = void 0;
      }
      if (errors === 0) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (data.preset === void 0 && (missing0 = "preset") || data.maxConcurrency === void 0 && (missing0 = "maxConcurrency") || data.admission === void 0 && (missing0 = "admission") || data.coordinatorAgentId === void 0 && (missing0 = "coordinatorAgentId") || data.memoryEnabled === void 0 && (missing0 = "memoryEnabled") || data.memoryConnectionId === void 0 && (missing0 = "memoryConnectionId") || data.holons === void 0 && (missing0 = "holons") || data.edges === void 0 && (missing0 = "edges") || data.positions === void 0 && (missing0 = "positions") || data.messageHopLimit === void 0 && (missing0 = "messageHopLimit") || data.maxMessagesPerRun === void 0 && (missing0 = "maxMessagesPerRun") || data.paused === void 0 && (missing0 = "paused")) {
            validate21.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
            return false;
          } else {
            const _errs1 = errors;
            for (const key0 in data) {
              if (!func1.call(schema34.properties, key0)) {
                validate21.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
                return false;
                break;
              }
            }
            if (_errs1 === errors) {
              if (data.preset !== void 0) {
                let data0 = data.preset;
                const _errs2 = errors;
                if (!(data0 === "single" || data0 === "independent" || data0 === "network" || data0 === "star" || data0 === "holonic" || data0 === "custom")) {
                  validate21.errors = [{ instancePath: instancePath + "/preset", schemaPath: "#/properties/preset/enum", keyword: "enum", params: { allowedValues: schema34.properties.preset.enum }, message: "must be equal to one of the allowed values" }];
                  return false;
                }
                var valid0 = _errs2 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.maxConcurrency !== void 0) {
                  let data1 = data.maxConcurrency;
                  const _errs3 = errors;
                  if (!(typeof data1 == "number" && (!(data1 % 1) && !isNaN(data1)) && isFinite(data1))) {
                    validate21.errors = [{ instancePath: instancePath + "/maxConcurrency", schemaPath: "#/properties/maxConcurrency/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                    return false;
                  }
                  if (errors === _errs3) {
                    if (typeof data1 == "number" && isFinite(data1)) {
                      if (data1 > 5e3 || isNaN(data1)) {
                        validate21.errors = [{ instancePath: instancePath + "/maxConcurrency", schemaPath: "#/properties/maxConcurrency/maximum", keyword: "maximum", params: { comparison: "<=", limit: 5e3 }, message: "must be <= 5000" }];
                        return false;
                      } else {
                        if (data1 < 1 || isNaN(data1)) {
                          validate21.errors = [{ instancePath: instancePath + "/maxConcurrency", schemaPath: "#/properties/maxConcurrency/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                          return false;
                        }
                      }
                    }
                  }
                  var valid0 = _errs3 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.admission !== void 0) {
                    let data2 = data.admission;
                    const _errs5 = errors;
                    if (!(data2 === "queue_allowed" || data2 === "start_all")) {
                      validate21.errors = [{ instancePath: instancePath + "/admission", schemaPath: "#/properties/admission/enum", keyword: "enum", params: { allowedValues: schema34.properties.admission.enum }, message: "must be equal to one of the allowed values" }];
                      return false;
                    }
                    var valid0 = _errs5 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.coordinatorAgentId !== void 0) {
                      let data3 = data.coordinatorAgentId;
                      const _errs6 = errors;
                      if (typeof data3 !== "string" && data3 !== null) {
                        validate21.errors = [{ instancePath: instancePath + "/coordinatorAgentId", schemaPath: "#/properties/coordinatorAgentId/type", keyword: "type", params: { type: schema34.properties.coordinatorAgentId.type }, message: "must be string,null" }];
                        return false;
                      }
                      var valid0 = _errs6 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.memoryEnabled !== void 0) {
                        const _errs8 = errors;
                        if (typeof data.memoryEnabled !== "boolean") {
                          validate21.errors = [{ instancePath: instancePath + "/memoryEnabled", schemaPath: "#/properties/memoryEnabled/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                          return false;
                        }
                        var valid0 = _errs8 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.memoryConnectionId !== void 0) {
                          let data5 = data.memoryConnectionId;
                          const _errs10 = errors;
                          if (typeof data5 !== "string" && data5 !== null) {
                            validate21.errors = [{ instancePath: instancePath + "/memoryConnectionId", schemaPath: "#/properties/memoryConnectionId/type", keyword: "type", params: { type: schema34.properties.memoryConnectionId.type }, message: "must be string,null" }];
                            return false;
                          }
                          var valid0 = _errs10 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.holons !== void 0) {
                            let data6 = data.holons;
                            const _errs12 = errors;
                            if (errors === _errs12) {
                              if (Array.isArray(data6)) {
                                if (data6.length > 5e3) {
                                  validate21.errors = [{ instancePath: instancePath + "/holons", schemaPath: "#/properties/holons/maxItems", keyword: "maxItems", params: { limit: 5e3 }, message: "must NOT have more than 5000 items" }];
                                  return false;
                                } else {
                                  var valid1 = true;
                                  const len0 = data6.length;
                                  for (let i0 = 0; i0 < len0; i0++) {
                                    let data7 = data6[i0];
                                    const _errs14 = errors;
                                    const _errs15 = errors;
                                    if (errors === _errs15) {
                                      if (data7 && typeof data7 == "object" && !Array.isArray(data7)) {
                                        let missing1;
                                        if (data7.id === void 0 && (missing1 = "id") || data7.name === void 0 && (missing1 = "name") || data7.goal === void 0 && (missing1 = "goal") || data7.parentId === void 0 && (missing1 = "parentId") || data7.members === void 0 && (missing1 = "members") || data7.coordinatorAgentId === void 0 && (missing1 = "coordinatorAgentId") || data7.inputContract === void 0 && (missing1 = "inputContract") || data7.outputContract === void 0 && (missing1 = "outputContract") || data7.maxTasks === void 0 && (missing1 = "maxTasks")) {
                                          validate21.errors = [{ instancePath: instancePath + "/holons/" + i0, schemaPath: "#/$defs/holon/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                                          return false;
                                        } else {
                                          const _errs17 = errors;
                                          for (const key1 in data7) {
                                            if (!func1.call(schema35.properties, key1)) {
                                              validate21.errors = [{ instancePath: instancePath + "/holons/" + i0, schemaPath: "#/$defs/holon/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
                                              return false;
                                              break;
                                            }
                                          }
                                          if (_errs17 === errors) {
                                            if (data7.id !== void 0) {
                                              const _errs18 = errors;
                                              if (typeof data7.id !== "string") {
                                                validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/id", schemaPath: "#/$defs/holon/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                return false;
                                              }
                                              var valid3 = _errs18 === errors;
                                            } else {
                                              var valid3 = true;
                                            }
                                            if (valid3) {
                                              if (data7.name !== void 0) {
                                                const _errs20 = errors;
                                                if (typeof data7.name !== "string") {
                                                  validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/name", schemaPath: "#/$defs/holon/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                  return false;
                                                }
                                                var valid3 = _errs20 === errors;
                                              } else {
                                                var valid3 = true;
                                              }
                                              if (valid3) {
                                                if (data7.goal !== void 0) {
                                                  const _errs22 = errors;
                                                  if (typeof data7.goal !== "string") {
                                                    validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/goal", schemaPath: "#/$defs/holon/properties/goal/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                    return false;
                                                  }
                                                  var valid3 = _errs22 === errors;
                                                } else {
                                                  var valid3 = true;
                                                }
                                                if (valid3) {
                                                  if (data7.parentId !== void 0) {
                                                    let data11 = data7.parentId;
                                                    const _errs24 = errors;
                                                    if (typeof data11 !== "string" && data11 !== null) {
                                                      validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/parentId", schemaPath: "#/$defs/holon/properties/parentId/type", keyword: "type", params: { type: schema35.properties.parentId.type }, message: "must be string,null" }];
                                                      return false;
                                                    }
                                                    var valid3 = _errs24 === errors;
                                                  } else {
                                                    var valid3 = true;
                                                  }
                                                  if (valid3) {
                                                    if (data7.members !== void 0) {
                                                      let data12 = data7.members;
                                                      const _errs26 = errors;
                                                      if (errors === _errs26) {
                                                        if (Array.isArray(data12)) {
                                                          if (data12.length > 5e3) {
                                                            validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/members", schemaPath: "#/$defs/holon/properties/members/maxItems", keyword: "maxItems", params: { limit: 5e3 }, message: "must NOT have more than 5000 items" }];
                                                            return false;
                                                          } else {
                                                            var valid4 = true;
                                                            const len1 = data12.length;
                                                            for (let i1 = 0; i1 < len1; i1++) {
                                                              const _errs28 = errors;
                                                              if (typeof data12[i1] !== "string") {
                                                                validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/members/" + i1, schemaPath: "#/$defs/holon/properties/members/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                                return false;
                                                              }
                                                              var valid4 = _errs28 === errors;
                                                              if (!valid4) {
                                                                break;
                                                              }
                                                            }
                                                            if (valid4) {
                                                              let i2 = data12.length;
                                                              let j0;
                                                              if (i2 > 1) {
                                                                const indices0 = {};
                                                                for (; i2--; ) {
                                                                  let item0 = data12[i2];
                                                                  if (typeof item0 !== "string") {
                                                                    continue;
                                                                  }
                                                                  if (typeof indices0[item0] == "number") {
                                                                    j0 = indices0[item0];
                                                                    validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/members", schemaPath: "#/$defs/holon/properties/members/uniqueItems", keyword: "uniqueItems", params: { i: i2, j: j0 }, message: "must NOT have duplicate items (items ## " + j0 + " and " + i2 + " are identical)" }];
                                                                    return false;
                                                                    break;
                                                                  }
                                                                  indices0[item0] = i2;
                                                                }
                                                              }
                                                            }
                                                          }
                                                        } else {
                                                          validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/members", schemaPath: "#/$defs/holon/properties/members/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                                          return false;
                                                        }
                                                      }
                                                      var valid3 = _errs26 === errors;
                                                    } else {
                                                      var valid3 = true;
                                                    }
                                                    if (valid3) {
                                                      if (data7.coordinatorAgentId !== void 0) {
                                                        let data14 = data7.coordinatorAgentId;
                                                        const _errs30 = errors;
                                                        if (typeof data14 !== "string" && data14 !== null) {
                                                          validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/coordinatorAgentId", schemaPath: "#/$defs/holon/properties/coordinatorAgentId/type", keyword: "type", params: { type: schema35.properties.coordinatorAgentId.type }, message: "must be string,null" }];
                                                          return false;
                                                        }
                                                        var valid3 = _errs30 === errors;
                                                      } else {
                                                        var valid3 = true;
                                                      }
                                                      if (valid3) {
                                                        if (data7.inputContract !== void 0) {
                                                          const _errs32 = errors;
                                                          if (typeof data7.inputContract !== "string") {
                                                            validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/inputContract", schemaPath: "#/$defs/holon/properties/inputContract/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                            return false;
                                                          }
                                                          var valid3 = _errs32 === errors;
                                                        } else {
                                                          var valid3 = true;
                                                        }
                                                        if (valid3) {
                                                          if (data7.outputContract !== void 0) {
                                                            const _errs34 = errors;
                                                            if (typeof data7.outputContract !== "string") {
                                                              validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/outputContract", schemaPath: "#/$defs/holon/properties/outputContract/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                              return false;
                                                            }
                                                            var valid3 = _errs34 === errors;
                                                          } else {
                                                            var valid3 = true;
                                                          }
                                                          if (valid3) {
                                                            if (data7.maxTasks !== void 0) {
                                                              let data17 = data7.maxTasks;
                                                              const _errs36 = errors;
                                                              if (!(typeof data17 == "number" && (!(data17 % 1) && !isNaN(data17)) && isFinite(data17))) {
                                                                validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/maxTasks", schemaPath: "#/$defs/holon/properties/maxTasks/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                                                return false;
                                                              }
                                                              if (errors === _errs36) {
                                                                if (typeof data17 == "number" && isFinite(data17)) {
                                                                  if (data17 < 0 || isNaN(data17)) {
                                                                    validate21.errors = [{ instancePath: instancePath + "/holons/" + i0 + "/maxTasks", schemaPath: "#/$defs/holon/properties/maxTasks/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                                    return false;
                                                                  }
                                                                }
                                                              }
                                                              var valid3 = _errs36 === errors;
                                                            } else {
                                                              var valid3 = true;
                                                            }
                                                          }
                                                        }
                                                      }
                                                    }
                                                  }
                                                }
                                              }
                                            }
                                          }
                                        }
                                      } else {
                                        validate21.errors = [{ instancePath: instancePath + "/holons/" + i0, schemaPath: "#/$defs/holon/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                                        return false;
                                      }
                                    }
                                    var valid1 = _errs14 === errors;
                                    if (!valid1) {
                                      break;
                                    }
                                  }
                                }
                              } else {
                                validate21.errors = [{ instancePath: instancePath + "/holons", schemaPath: "#/properties/holons/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                return false;
                              }
                            }
                            var valid0 = _errs12 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.edges !== void 0) {
                              let data18 = data.edges;
                              const _errs38 = errors;
                              if (errors === _errs38) {
                                if (Array.isArray(data18)) {
                                  if (data18.length > 25e3) {
                                    validate21.errors = [{ instancePath: instancePath + "/edges", schemaPath: "#/properties/edges/maxItems", keyword: "maxItems", params: { limit: 25e3 }, message: "must NOT have more than 25000 items" }];
                                    return false;
                                  } else {
                                    var valid6 = true;
                                    const len2 = data18.length;
                                    for (let i3 = 0; i3 < len2; i3++) {
                                      let data19 = data18[i3];
                                      const _errs40 = errors;
                                      if (errors === _errs40) {
                                        if (data19 && typeof data19 == "object" && !Array.isArray(data19)) {
                                          let missing2;
                                          if (data19.from === void 0 && (missing2 = "from") || data19.to === void 0 && (missing2 = "to")) {
                                            validate21.errors = [{ instancePath: instancePath + "/edges/" + i3, schemaPath: "#/properties/edges/items/required", keyword: "required", params: { missingProperty: missing2 }, message: "must have required property '" + missing2 + "'" }];
                                            return false;
                                          } else {
                                            const _errs42 = errors;
                                            for (const key2 in data19) {
                                              if (!(key2 === "from" || key2 === "to")) {
                                                validate21.errors = [{ instancePath: instancePath + "/edges/" + i3, schemaPath: "#/properties/edges/items/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" }];
                                                return false;
                                                break;
                                              }
                                            }
                                            if (_errs42 === errors) {
                                              if (data19.from !== void 0) {
                                                const _errs43 = errors;
                                                if (typeof data19.from !== "string") {
                                                  validate21.errors = [{ instancePath: instancePath + "/edges/" + i3 + "/from", schemaPath: "#/properties/edges/items/properties/from/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                  return false;
                                                }
                                                var valid7 = _errs43 === errors;
                                              } else {
                                                var valid7 = true;
                                              }
                                              if (valid7) {
                                                if (data19.to !== void 0) {
                                                  const _errs45 = errors;
                                                  if (typeof data19.to !== "string") {
                                                    validate21.errors = [{ instancePath: instancePath + "/edges/" + i3 + "/to", schemaPath: "#/properties/edges/items/properties/to/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                    return false;
                                                  }
                                                  var valid7 = _errs45 === errors;
                                                } else {
                                                  var valid7 = true;
                                                }
                                              }
                                            }
                                          }
                                        } else {
                                          validate21.errors = [{ instancePath: instancePath + "/edges/" + i3, schemaPath: "#/properties/edges/items/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                                          return false;
                                        }
                                      }
                                      var valid6 = _errs40 === errors;
                                      if (!valid6) {
                                        break;
                                      }
                                    }
                                  }
                                } else {
                                  validate21.errors = [{ instancePath: instancePath + "/edges", schemaPath: "#/properties/edges/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                  return false;
                                }
                              }
                              var valid0 = _errs38 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.positions !== void 0) {
                                let data22 = data.positions;
                                const _errs47 = errors;
                                if (errors === _errs47) {
                                  if (data22 && typeof data22 == "object" && !Array.isArray(data22)) {
                                    for (const key3 in data22) {
                                      let data23 = data22[key3];
                                      const _errs50 = errors;
                                      if (errors === _errs50) {
                                        if (Array.isArray(data23)) {
                                          if (data23.length > 2) {
                                            validate21.errors = [{ instancePath: instancePath + "/positions/" + key3.replace(/~/g, "~0").replace(/\//g, "~1"), schemaPath: "#/properties/positions/additionalProperties/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" }];
                                            return false;
                                          } else {
                                            if (data23.length < 2) {
                                              validate21.errors = [{ instancePath: instancePath + "/positions/" + key3.replace(/~/g, "~0").replace(/\//g, "~1"), schemaPath: "#/properties/positions/additionalProperties/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" }];
                                              return false;
                                            } else {
                                              var valid9 = true;
                                              const len3 = data23.length;
                                              for (let i4 = 0; i4 < len3; i4++) {
                                                let data24 = data23[i4];
                                                const _errs52 = errors;
                                                if (errors === _errs52) {
                                                  if (typeof data24 == "number" && isFinite(data24)) {
                                                    if (data24 > 999999 || isNaN(data24)) {
                                                      validate21.errors = [{ instancePath: instancePath + "/positions/" + key3.replace(/~/g, "~0").replace(/\//g, "~1") + "/" + i4, schemaPath: "#/properties/positions/additionalProperties/items/maximum", keyword: "maximum", params: { comparison: "<=", limit: 999999 }, message: "must be <= 999999" }];
                                                      return false;
                                                    } else {
                                                      if (data24 < -999999 || isNaN(data24)) {
                                                        validate21.errors = [{ instancePath: instancePath + "/positions/" + key3.replace(/~/g, "~0").replace(/\//g, "~1") + "/" + i4, schemaPath: "#/properties/positions/additionalProperties/items/minimum", keyword: "minimum", params: { comparison: ">=", limit: -999999 }, message: "must be >= -999999" }];
                                                        return false;
                                                      }
                                                    }
                                                  } else {
                                                    validate21.errors = [{ instancePath: instancePath + "/positions/" + key3.replace(/~/g, "~0").replace(/\//g, "~1") + "/" + i4, schemaPath: "#/properties/positions/additionalProperties/items/type", keyword: "type", params: { type: "number" }, message: "must be number" }];
                                                    return false;
                                                  }
                                                }
                                                var valid9 = _errs52 === errors;
                                                if (!valid9) {
                                                  break;
                                                }
                                              }
                                            }
                                          }
                                        } else {
                                          validate21.errors = [{ instancePath: instancePath + "/positions/" + key3.replace(/~/g, "~0").replace(/\//g, "~1"), schemaPath: "#/properties/positions/additionalProperties/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                          return false;
                                        }
                                      }
                                      var valid8 = _errs50 === errors;
                                      if (!valid8) {
                                        break;
                                      }
                                    }
                                  } else {
                                    validate21.errors = [{ instancePath: instancePath + "/positions", schemaPath: "#/properties/positions/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                                    return false;
                                  }
                                }
                                var valid0 = _errs47 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.messageHopLimit !== void 0) {
                                  let data25 = data.messageHopLimit;
                                  const _errs54 = errors;
                                  if (!(typeof data25 == "number" && (!(data25 % 1) && !isNaN(data25)) && isFinite(data25))) {
                                    validate21.errors = [{ instancePath: instancePath + "/messageHopLimit", schemaPath: "#/properties/messageHopLimit/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                    return false;
                                  }
                                  if (errors === _errs54) {
                                    if (typeof data25 == "number" && isFinite(data25)) {
                                      if (data25 > 32 || isNaN(data25)) {
                                        validate21.errors = [{ instancePath: instancePath + "/messageHopLimit", schemaPath: "#/properties/messageHopLimit/maximum", keyword: "maximum", params: { comparison: "<=", limit: 32 }, message: "must be <= 32" }];
                                        return false;
                                      } else {
                                        if (data25 < 1 || isNaN(data25)) {
                                          validate21.errors = [{ instancePath: instancePath + "/messageHopLimit", schemaPath: "#/properties/messageHopLimit/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                                          return false;
                                        }
                                      }
                                    }
                                  }
                                  var valid0 = _errs54 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.communicationMode !== void 0) {
                                    let data26 = data.communicationMode;
                                    const _errs56 = errors;
                                    if (!(data26 === "direct" || data26 === "blackboard" || data26 === "wiki" || data26 === "graph")) {
                                      validate21.errors = [{ instancePath: instancePath + "/communicationMode", schemaPath: "#/properties/communicationMode/enum", keyword: "enum", params: { allowedValues: schema34.properties.communicationMode.enum }, message: "must be equal to one of the allowed values" }];
                                      return false;
                                    }
                                    var valid0 = _errs56 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.maxMessagesPerRun !== void 0) {
                                      let data27 = data.maxMessagesPerRun;
                                      const _errs57 = errors;
                                      if (!(typeof data27 == "number" && (!(data27 % 1) && !isNaN(data27)) && isFinite(data27))) {
                                        validate21.errors = [{ instancePath: instancePath + "/maxMessagesPerRun", schemaPath: "#/properties/maxMessagesPerRun/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                        return false;
                                      }
                                      if (errors === _errs57) {
                                        if (typeof data27 == "number" && isFinite(data27)) {
                                          if (data27 > 1e5 || isNaN(data27)) {
                                            validate21.errors = [{ instancePath: instancePath + "/maxMessagesPerRun", schemaPath: "#/properties/maxMessagesPerRun/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1e5 }, message: "must be <= 100000" }];
                                            return false;
                                          } else {
                                            if (data27 < 1 || isNaN(data27)) {
                                              validate21.errors = [{ instancePath: instancePath + "/maxMessagesPerRun", schemaPath: "#/properties/maxMessagesPerRun/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                                              return false;
                                            }
                                          }
                                        }
                                      }
                                      var valid0 = _errs57 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.paused !== void 0) {
                                        const _errs59 = errors;
                                        if (true !== data.paused) {
                                          validate21.errors = [{ instancePath: instancePath + "/paused", schemaPath: "#/properties/paused/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                                          return false;
                                        }
                                        var valid0 = _errs59 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        } else {
          validate21.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
          return false;
        }
      }
      validate21.errors = vErrors;
      return errors === 0;
    }
    validate21.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
    function validate20(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
      ;
      let vErrors = null;
      let errors = 0;
      const evaluated0 = validate20.evaluated;
      if (evaluated0.dynamicProps) {
        evaluated0.props = void 0;
      }
      if (evaluated0.dynamicItems) {
        evaluated0.items = void 0;
      }
      if (errors === 0) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (data.format === void 0 && (missing0 = "format") || data.schemaVersion === void 0 && (missing0 = "schemaVersion") || data.migrationVersion === void 0 && (missing0 = "migrationVersion") || data.sourceRevision === void 0 && (missing0 = "sourceRevision") || data.name === void 0 && (missing0 = "name") || data.runtime === void 0 && (missing0 = "runtime") || data.roleVersions === void 0 && (missing0 = "roleVersions") || data.instances === void 0 && (missing0 = "instances") || data.architecture === void 0 && (missing0 = "architecture") || data.connections === void 0 && (missing0 = "connections") || data.bindings === void 0 && (missing0 = "bindings")) {
            validate20.errors = [{ instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: missing0 }, message: "must have required property '" + missing0 + "'" }];
            return false;
          } else {
            const _errs1 = errors;
            for (const key0 in data) {
              if (!func1.call(schema31.properties, key0)) {
                validate20.errors = [{ instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" }];
                return false;
                break;
              }
            }
            if (_errs1 === errors) {
              if (data.format !== void 0) {
                const _errs2 = errors;
                if ("polymas-configuration" !== data.format) {
                  validate20.errors = [{ instancePath: instancePath + "/format", schemaPath: "#/properties/format/const", keyword: "const", params: { allowedValue: "polymas-configuration" }, message: "must be equal to constant" }];
                  return false;
                }
                var valid0 = _errs2 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.schemaVersion !== void 0) {
                  const _errs3 = errors;
                  if ("0.1.0" !== data.schemaVersion) {
                    validate20.errors = [{ instancePath: instancePath + "/schemaVersion", schemaPath: "#/properties/schemaVersion/const", keyword: "const", params: { allowedValue: "0.1.0" }, message: "must be equal to constant" }];
                    return false;
                  }
                  var valid0 = _errs3 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.migrationVersion !== void 0) {
                    const _errs4 = errors;
                    if (1 !== data.migrationVersion) {
                      validate20.errors = [{ instancePath: instancePath + "/migrationVersion", schemaPath: "#/properties/migrationVersion/const", keyword: "const", params: { allowedValue: 1 }, message: "must be equal to constant" }];
                      return false;
                    }
                    var valid0 = _errs4 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.sourceRevision !== void 0) {
                      let data3 = data.sourceRevision;
                      const _errs5 = errors;
                      if (!(typeof data3 == "number" && (!(data3 % 1) && !isNaN(data3)) && isFinite(data3))) {
                        validate20.errors = [{ instancePath: instancePath + "/sourceRevision", schemaPath: "#/properties/sourceRevision/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                        return false;
                      }
                      if (errors === _errs5) {
                        if (typeof data3 == "number" && isFinite(data3)) {
                          if (data3 < 1 || isNaN(data3)) {
                            validate20.errors = [{ instancePath: instancePath + "/sourceRevision", schemaPath: "#/properties/sourceRevision/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                            return false;
                          }
                        }
                      }
                      var valid0 = _errs5 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.name !== void 0) {
                        const _errs7 = errors;
                        if (typeof data.name !== "string") {
                          validate20.errors = [{ instancePath: instancePath + "/name", schemaPath: "#/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                          return false;
                        }
                        var valid0 = _errs7 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.runtime !== void 0) {
                          let data5 = data.runtime;
                          const _errs9 = errors;
                          if (errors === _errs9) {
                            if (data5 && typeof data5 == "object" && !Array.isArray(data5)) {
                              let missing1;
                              if (data5.adapter === void 0 && (missing1 = "adapter") || data5.setupRequired === void 0 && (missing1 = "setupRequired")) {
                                validate20.errors = [{ instancePath: instancePath + "/runtime", schemaPath: "#/properties/runtime/required", keyword: "required", params: { missingProperty: missing1 }, message: "must have required property '" + missing1 + "'" }];
                                return false;
                              } else {
                                const _errs11 = errors;
                                for (const key1 in data5) {
                                  if (!(key1 === "adapter" || key1 === "setupRequired")) {
                                    validate20.errors = [{ instancePath: instancePath + "/runtime", schemaPath: "#/properties/runtime/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" }];
                                    return false;
                                    break;
                                  }
                                }
                                if (_errs11 === errors) {
                                  if (data5.adapter !== void 0) {
                                    const _errs12 = errors;
                                    if ("codex-exec" !== data5.adapter) {
                                      validate20.errors = [{ instancePath: instancePath + "/runtime/adapter", schemaPath: "#/properties/runtime/properties/adapter/const", keyword: "const", params: { allowedValue: "codex-exec" }, message: "must be equal to constant" }];
                                      return false;
                                    }
                                    var valid1 = _errs12 === errors;
                                  } else {
                                    var valid1 = true;
                                  }
                                  if (valid1) {
                                    if (data5.setupRequired !== void 0) {
                                      const _errs13 = errors;
                                      if (true !== data5.setupRequired) {
                                        validate20.errors = [{ instancePath: instancePath + "/runtime/setupRequired", schemaPath: "#/properties/runtime/properties/setupRequired/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                                        return false;
                                      }
                                      var valid1 = _errs13 === errors;
                                    } else {
                                      var valid1 = true;
                                    }
                                  }
                                }
                              }
                            } else {
                              validate20.errors = [{ instancePath: instancePath + "/runtime", schemaPath: "#/properties/runtime/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                              return false;
                            }
                          }
                          var valid0 = _errs9 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.roleVersions !== void 0) {
                            let data8 = data.roleVersions;
                            const _errs14 = errors;
                            if (errors === _errs14) {
                              if (Array.isArray(data8)) {
                                if (data8.length > 1e5) {
                                  validate20.errors = [{ instancePath: instancePath + "/roleVersions", schemaPath: "#/properties/roleVersions/maxItems", keyword: "maxItems", params: { limit: 1e5 }, message: "must NOT have more than 100000 items" }];
                                  return false;
                                } else {
                                  var valid2 = true;
                                  const len0 = data8.length;
                                  for (let i0 = 0; i0 < len0; i0++) {
                                    let data9 = data8[i0];
                                    const _errs16 = errors;
                                    const _errs17 = errors;
                                    if (errors === _errs17) {
                                      if (data9 && typeof data9 == "object" && !Array.isArray(data9)) {
                                        let missing2;
                                        if (data9.id === void 0 && (missing2 = "id") || data9.version === void 0 && (missing2 = "version") || data9.name === void 0 && (missing2 = "name") || data9.description === void 0 && (missing2 = "description") || data9.instruction === void 0 && (missing2 = "instruction") || data9.outputContract === void 0 && (missing2 = "outputContract") || data9.acceptanceCriteria === void 0 && (missing2 = "acceptanceCriteria") || data9.escalationRule === void 0 && (missing2 = "escalationRule") || data9.model === void 0 && (missing2 = "model") || data9.sandbox === void 0 && (missing2 = "sandbox") || data9.timeoutSeconds === void 0 && (missing2 = "timeoutSeconds") || data9.maxTurns === void 0 && (missing2 = "maxTurns") || data9.maxOutputTokens === void 0 && (missing2 = "maxOutputTokens")) {
                                          validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0, schemaPath: "#/$defs/role/required", keyword: "required", params: { missingProperty: missing2 }, message: "must have required property '" + missing2 + "'" }];
                                          return false;
                                        } else {
                                          const _errs19 = errors;
                                          for (const key2 in data9) {
                                            if (!func1.call(schema32.properties, key2)) {
                                              validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0, schemaPath: "#/$defs/role/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" }];
                                              return false;
                                              break;
                                            }
                                          }
                                          if (_errs19 === errors) {
                                            if (data9.id !== void 0) {
                                              const _errs20 = errors;
                                              if (typeof data9.id !== "string") {
                                                validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/id", schemaPath: "#/$defs/role/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                return false;
                                              }
                                              var valid4 = _errs20 === errors;
                                            } else {
                                              var valid4 = true;
                                            }
                                            if (valid4) {
                                              if (data9.version !== void 0) {
                                                let data11 = data9.version;
                                                const _errs22 = errors;
                                                if (!(typeof data11 == "number" && (!(data11 % 1) && !isNaN(data11)) && isFinite(data11))) {
                                                  validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/version", schemaPath: "#/$defs/role/properties/version/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                                  return false;
                                                }
                                                if (errors === _errs22) {
                                                  if (typeof data11 == "number" && isFinite(data11)) {
                                                    if (data11 < 1 || isNaN(data11)) {
                                                      validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/version", schemaPath: "#/$defs/role/properties/version/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                                                      return false;
                                                    }
                                                  }
                                                }
                                                var valid4 = _errs22 === errors;
                                              } else {
                                                var valid4 = true;
                                              }
                                              if (valid4) {
                                                if (data9.name !== void 0) {
                                                  const _errs24 = errors;
                                                  if (typeof data9.name !== "string") {
                                                    validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/name", schemaPath: "#/$defs/role/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                    return false;
                                                  }
                                                  var valid4 = _errs24 === errors;
                                                } else {
                                                  var valid4 = true;
                                                }
                                                if (valid4) {
                                                  if (data9.description !== void 0) {
                                                    const _errs26 = errors;
                                                    if (typeof data9.description !== "string") {
                                                      validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/description", schemaPath: "#/$defs/role/properties/description/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                      return false;
                                                    }
                                                    var valid4 = _errs26 === errors;
                                                  } else {
                                                    var valid4 = true;
                                                  }
                                                  if (valid4) {
                                                    if (data9.instruction !== void 0) {
                                                      const _errs28 = errors;
                                                      if (typeof data9.instruction !== "string") {
                                                        validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/instruction", schemaPath: "#/$defs/role/properties/instruction/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                        return false;
                                                      }
                                                      var valid4 = _errs28 === errors;
                                                    } else {
                                                      var valid4 = true;
                                                    }
                                                    if (valid4) {
                                                      if (data9.outputContract !== void 0) {
                                                        const _errs30 = errors;
                                                        if (typeof data9.outputContract !== "string") {
                                                          validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/outputContract", schemaPath: "#/$defs/role/properties/outputContract/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                          return false;
                                                        }
                                                        var valid4 = _errs30 === errors;
                                                      } else {
                                                        var valid4 = true;
                                                      }
                                                      if (valid4) {
                                                        if (data9.acceptanceCriteria !== void 0) {
                                                          const _errs32 = errors;
                                                          if (typeof data9.acceptanceCriteria !== "string") {
                                                            validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/acceptanceCriteria", schemaPath: "#/$defs/role/properties/acceptanceCriteria/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                            return false;
                                                          }
                                                          var valid4 = _errs32 === errors;
                                                        } else {
                                                          var valid4 = true;
                                                        }
                                                        if (valid4) {
                                                          if (data9.escalationRule !== void 0) {
                                                            const _errs34 = errors;
                                                            if (typeof data9.escalationRule !== "string") {
                                                              validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/escalationRule", schemaPath: "#/$defs/role/properties/escalationRule/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                              return false;
                                                            }
                                                            var valid4 = _errs34 === errors;
                                                          } else {
                                                            var valid4 = true;
                                                          }
                                                          if (valid4) {
                                                            if (data9.model !== void 0) {
                                                              let data18 = data9.model;
                                                              const _errs36 = errors;
                                                              if (typeof data18 !== "string" && data18 !== null) {
                                                                validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/model", schemaPath: "#/$defs/role/properties/model/type", keyword: "type", params: { type: schema32.properties.model.type }, message: "must be string,null" }];
                                                                return false;
                                                              }
                                                              var valid4 = _errs36 === errors;
                                                            } else {
                                                              var valid4 = true;
                                                            }
                                                            if (valid4) {
                                                              if (data9.sandbox !== void 0) {
                                                                let data19 = data9.sandbox;
                                                                const _errs38 = errors;
                                                                if (!(data19 === "read-only" || data19 === "workspace-write")) {
                                                                  validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/sandbox", schemaPath: "#/$defs/role/properties/sandbox/enum", keyword: "enum", params: { allowedValues: schema32.properties.sandbox.enum }, message: "must be equal to one of the allowed values" }];
                                                                  return false;
                                                                }
                                                                var valid4 = _errs38 === errors;
                                                              } else {
                                                                var valid4 = true;
                                                              }
                                                              if (valid4) {
                                                                if (data9.timeoutSeconds !== void 0) {
                                                                  let data20 = data9.timeoutSeconds;
                                                                  const _errs39 = errors;
                                                                  if (!(typeof data20 == "number" && (!(data20 % 1) && !isNaN(data20)) && isFinite(data20))) {
                                                                    validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/timeoutSeconds", schemaPath: "#/$defs/role/properties/timeoutSeconds/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                                                    return false;
                                                                  }
                                                                  if (errors === _errs39) {
                                                                    if (typeof data20 == "number" && isFinite(data20)) {
                                                                      if (data20 > 86400 || isNaN(data20)) {
                                                                        validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/timeoutSeconds", schemaPath: "#/$defs/role/properties/timeoutSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 86400 }, message: "must be <= 86400" }];
                                                                        return false;
                                                                      } else {
                                                                        if (data20 < 5 || isNaN(data20)) {
                                                                          validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/timeoutSeconds", schemaPath: "#/$defs/role/properties/timeoutSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 5 }, message: "must be >= 5" }];
                                                                          return false;
                                                                        }
                                                                      }
                                                                    }
                                                                  }
                                                                  var valid4 = _errs39 === errors;
                                                                } else {
                                                                  var valid4 = true;
                                                                }
                                                                if (valid4) {
                                                                  if (data9.maxTurns !== void 0) {
                                                                    let data21 = data9.maxTurns;
                                                                    const _errs41 = errors;
                                                                    if (!(typeof data21 == "number" && (!(data21 % 1) && !isNaN(data21)) && isFinite(data21))) {
                                                                      validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/maxTurns", schemaPath: "#/$defs/role/properties/maxTurns/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                                                      return false;
                                                                    }
                                                                    if (errors === _errs41) {
                                                                      if (typeof data21 == "number" && isFinite(data21)) {
                                                                        if (data21 > 1e3 || isNaN(data21)) {
                                                                          validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/maxTurns", schemaPath: "#/$defs/role/properties/maxTurns/maximum", keyword: "maximum", params: { comparison: "<=", limit: 1e3 }, message: "must be <= 1000" }];
                                                                          return false;
                                                                        } else {
                                                                          if (data21 < 1 || isNaN(data21)) {
                                                                            validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/maxTurns", schemaPath: "#/$defs/role/properties/maxTurns/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                                                                            return false;
                                                                          }
                                                                        }
                                                                      }
                                                                    }
                                                                    var valid4 = _errs41 === errors;
                                                                  } else {
                                                                    var valid4 = true;
                                                                  }
                                                                  if (valid4) {
                                                                    if (data9.maxOutputTokens !== void 0) {
                                                                      let data22 = data9.maxOutputTokens;
                                                                      const _errs43 = errors;
                                                                      if (!(typeof data22 == "number" && (!(data22 % 1) && !isNaN(data22)) && isFinite(data22)) && data22 !== null) {
                                                                        validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/maxOutputTokens", schemaPath: "#/$defs/role/properties/maxOutputTokens/type", keyword: "type", params: { type: schema32.properties.maxOutputTokens.type }, message: "must be integer,null" }];
                                                                        return false;
                                                                      }
                                                                      if (errors === _errs43) {
                                                                        if (typeof data22 == "number" && isFinite(data22)) {
                                                                          if (data22 < 0 || isNaN(data22)) {
                                                                            validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0 + "/maxOutputTokens", schemaPath: "#/$defs/role/properties/maxOutputTokens/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                                            return false;
                                                                          }
                                                                        }
                                                                      }
                                                                      var valid4 = _errs43 === errors;
                                                                    } else {
                                                                      var valid4 = true;
                                                                    }
                                                                  }
                                                                }
                                                              }
                                                            }
                                                          }
                                                        }
                                                      }
                                                    }
                                                  }
                                                }
                                              }
                                            }
                                          }
                                        }
                                      } else {
                                        validate20.errors = [{ instancePath: instancePath + "/roleVersions/" + i0, schemaPath: "#/$defs/role/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                                        return false;
                                      }
                                    }
                                    var valid2 = _errs16 === errors;
                                    if (!valid2) {
                                      break;
                                    }
                                  }
                                }
                              } else {
                                validate20.errors = [{ instancePath: instancePath + "/roleVersions", schemaPath: "#/properties/roleVersions/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                return false;
                              }
                            }
                            var valid0 = _errs14 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.instances !== void 0) {
                              let data23 = data.instances;
                              const _errs45 = errors;
                              if (errors === _errs45) {
                                if (Array.isArray(data23)) {
                                  if (data23.length > 1e5) {
                                    validate20.errors = [{ instancePath: instancePath + "/instances", schemaPath: "#/properties/instances/maxItems", keyword: "maxItems", params: { limit: 1e5 }, message: "must NOT have more than 100000 items" }];
                                    return false;
                                  } else {
                                    var valid5 = true;
                                    const len1 = data23.length;
                                    for (let i1 = 0; i1 < len1; i1++) {
                                      let data24 = data23[i1];
                                      const _errs47 = errors;
                                      const _errs48 = errors;
                                      if (errors === _errs48) {
                                        if (data24 && typeof data24 == "object" && !Array.isArray(data24)) {
                                          let missing3;
                                          if (data24.id === void 0 && (missing3 = "id") || data24.name === void 0 && (missing3 = "name") || data24.roleId === void 0 && (missing3 = "roleId") || data24.roleVersion === void 0 && (missing3 = "roleVersion") || data24.archived === void 0 && (missing3 = "archived")) {
                                            validate20.errors = [{ instancePath: instancePath + "/instances/" + i1, schemaPath: "#/$defs/instance/required", keyword: "required", params: { missingProperty: missing3 }, message: "must have required property '" + missing3 + "'" }];
                                            return false;
                                          } else {
                                            const _errs50 = errors;
                                            for (const key3 in data24) {
                                              if (!(key3 === "id" || key3 === "name" || key3 === "roleId" || key3 === "roleVersion" || key3 === "archived")) {
                                                validate20.errors = [{ instancePath: instancePath + "/instances/" + i1, schemaPath: "#/$defs/instance/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" }];
                                                return false;
                                                break;
                                              }
                                            }
                                            if (_errs50 === errors) {
                                              if (data24.id !== void 0) {
                                                const _errs51 = errors;
                                                if (typeof data24.id !== "string") {
                                                  validate20.errors = [{ instancePath: instancePath + "/instances/" + i1 + "/id", schemaPath: "#/$defs/instance/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                  return false;
                                                }
                                                var valid7 = _errs51 === errors;
                                              } else {
                                                var valid7 = true;
                                              }
                                              if (valid7) {
                                                if (data24.name !== void 0) {
                                                  const _errs53 = errors;
                                                  if (typeof data24.name !== "string") {
                                                    validate20.errors = [{ instancePath: instancePath + "/instances/" + i1 + "/name", schemaPath: "#/$defs/instance/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                    return false;
                                                  }
                                                  var valid7 = _errs53 === errors;
                                                } else {
                                                  var valid7 = true;
                                                }
                                                if (valid7) {
                                                  if (data24.roleId !== void 0) {
                                                    const _errs55 = errors;
                                                    if (typeof data24.roleId !== "string") {
                                                      validate20.errors = [{ instancePath: instancePath + "/instances/" + i1 + "/roleId", schemaPath: "#/$defs/instance/properties/roleId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                      return false;
                                                    }
                                                    var valid7 = _errs55 === errors;
                                                  } else {
                                                    var valid7 = true;
                                                  }
                                                  if (valid7) {
                                                    if (data24.roleVersion !== void 0) {
                                                      let data28 = data24.roleVersion;
                                                      const _errs57 = errors;
                                                      if (!(typeof data28 == "number" && (!(data28 % 1) && !isNaN(data28)) && isFinite(data28))) {
                                                        validate20.errors = [{ instancePath: instancePath + "/instances/" + i1 + "/roleVersion", schemaPath: "#/$defs/instance/properties/roleVersion/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                                        return false;
                                                      }
                                                      if (errors === _errs57) {
                                                        if (typeof data28 == "number" && isFinite(data28)) {
                                                          if (data28 < 1 || isNaN(data28)) {
                                                            validate20.errors = [{ instancePath: instancePath + "/instances/" + i1 + "/roleVersion", schemaPath: "#/$defs/instance/properties/roleVersion/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                                                            return false;
                                                          }
                                                        }
                                                      }
                                                      var valid7 = _errs57 === errors;
                                                    } else {
                                                      var valid7 = true;
                                                    }
                                                    if (valid7) {
                                                      if (data24.archived !== void 0) {
                                                        const _errs59 = errors;
                                                        if (typeof data24.archived !== "boolean") {
                                                          validate20.errors = [{ instancePath: instancePath + "/instances/" + i1 + "/archived", schemaPath: "#/$defs/instance/properties/archived/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                                          return false;
                                                        }
                                                        var valid7 = _errs59 === errors;
                                                      } else {
                                                        var valid7 = true;
                                                      }
                                                    }
                                                  }
                                                }
                                              }
                                            }
                                          }
                                        } else {
                                          validate20.errors = [{ instancePath: instancePath + "/instances/" + i1, schemaPath: "#/$defs/instance/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                                          return false;
                                        }
                                      }
                                      var valid5 = _errs47 === errors;
                                      if (!valid5) {
                                        break;
                                      }
                                    }
                                  }
                                } else {
                                  validate20.errors = [{ instancePath: instancePath + "/instances", schemaPath: "#/properties/instances/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                  return false;
                                }
                              }
                              var valid0 = _errs45 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.architecture !== void 0) {
                                const _errs61 = errors;
                                if (!validate21(data.architecture, { instancePath: instancePath + "/architecture", parentData: data, parentDataProperty: "architecture", rootData, dynamicAnchors })) {
                                  vErrors = vErrors === null ? validate21.errors : vErrors.concat(validate21.errors);
                                  errors = vErrors.length;
                                }
                                var valid0 = _errs61 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.connections !== void 0) {
                                  let data31 = data.connections;
                                  const _errs62 = errors;
                                  if (errors === _errs62) {
                                    if (Array.isArray(data31)) {
                                      if (data31.length > 1e3) {
                                        validate20.errors = [{ instancePath: instancePath + "/connections", schemaPath: "#/properties/connections/maxItems", keyword: "maxItems", params: { limit: 1e3 }, message: "must NOT have more than 1000 items" }];
                                        return false;
                                      } else {
                                        var valid8 = true;
                                        const len2 = data31.length;
                                        for (let i2 = 0; i2 < len2; i2++) {
                                          let data32 = data31[i2];
                                          const _errs64 = errors;
                                          const _errs65 = errors;
                                          if (errors === _errs65) {
                                            if (data32 && typeof data32 == "object" && !Array.isArray(data32)) {
                                              let missing4;
                                              if (data32.id === void 0 && (missing4 = "id") || data32.name === void 0 && (missing4 = "name") || data32.recipe === void 0 && (missing4 = "recipe") || data32.transport === void 0 && (missing4 = "transport") || data32.readOnly === void 0 && (missing4 = "readOnly") || data32.timeoutSeconds === void 0 && (missing4 = "timeoutSeconds") || data32.toolAllowlist === void 0 && (missing4 = "toolAllowlist") || data32.desiredEnabled === void 0 && (missing4 = "desiredEnabled") || data32.setupRequired === void 0 && (missing4 = "setupRequired") || data32.setupFields === void 0 && (missing4 = "setupFields") || data32.rootCount === void 0 && (missing4 = "rootCount")) {
                                                validate20.errors = [{ instancePath: instancePath + "/connections/" + i2, schemaPath: "#/$defs/connection/required", keyword: "required", params: { missingProperty: missing4 }, message: "must have required property '" + missing4 + "'" }];
                                                return false;
                                              } else {
                                                const _errs67 = errors;
                                                for (const key4 in data32) {
                                                  if (!func1.call(schema36.properties, key4)) {
                                                    validate20.errors = [{ instancePath: instancePath + "/connections/" + i2, schemaPath: "#/$defs/connection/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" }];
                                                    return false;
                                                    break;
                                                  }
                                                }
                                                if (_errs67 === errors) {
                                                  if (data32.id !== void 0) {
                                                    const _errs68 = errors;
                                                    if (typeof data32.id !== "string") {
                                                      validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/id", schemaPath: "#/$defs/connection/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                      return false;
                                                    }
                                                    var valid10 = _errs68 === errors;
                                                  } else {
                                                    var valid10 = true;
                                                  }
                                                  if (valid10) {
                                                    if (data32.name !== void 0) {
                                                      const _errs70 = errors;
                                                      if (typeof data32.name !== "string") {
                                                        validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/name", schemaPath: "#/$defs/connection/properties/name/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                        return false;
                                                      }
                                                      var valid10 = _errs70 === errors;
                                                    } else {
                                                      var valid10 = true;
                                                    }
                                                    if (valid10) {
                                                      if (data32.recipe !== void 0) {
                                                        const _errs72 = errors;
                                                        if (typeof data32.recipe !== "string") {
                                                          validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/recipe", schemaPath: "#/$defs/connection/properties/recipe/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                          return false;
                                                        }
                                                        var valid10 = _errs72 === errors;
                                                      } else {
                                                        var valid10 = true;
                                                      }
                                                      if (valid10) {
                                                        if (data32.transport !== void 0) {
                                                          let data36 = data32.transport;
                                                          const _errs74 = errors;
                                                          if (!(data36 === "files" || data36 === "stdio" || data36 === "http")) {
                                                            validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/transport", schemaPath: "#/$defs/connection/properties/transport/enum", keyword: "enum", params: { allowedValues: schema36.properties.transport.enum }, message: "must be equal to one of the allowed values" }];
                                                            return false;
                                                          }
                                                          var valid10 = _errs74 === errors;
                                                        } else {
                                                          var valid10 = true;
                                                        }
                                                        if (valid10) {
                                                          if (data32.readOnly !== void 0) {
                                                            const _errs75 = errors;
                                                            if (typeof data32.readOnly !== "boolean") {
                                                              validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/readOnly", schemaPath: "#/$defs/connection/properties/readOnly/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                                              return false;
                                                            }
                                                            var valid10 = _errs75 === errors;
                                                          } else {
                                                            var valid10 = true;
                                                          }
                                                          if (valid10) {
                                                            if (data32.timeoutSeconds !== void 0) {
                                                              let data38 = data32.timeoutSeconds;
                                                              const _errs77 = errors;
                                                              if (!(typeof data38 == "number" && (!(data38 % 1) && !isNaN(data38)) && isFinite(data38))) {
                                                                validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/timeoutSeconds", schemaPath: "#/$defs/connection/properties/timeoutSeconds/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                                                return false;
                                                              }
                                                              if (errors === _errs77) {
                                                                if (typeof data38 == "number" && isFinite(data38)) {
                                                                  if (data38 > 300 || isNaN(data38)) {
                                                                    validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/timeoutSeconds", schemaPath: "#/$defs/connection/properties/timeoutSeconds/maximum", keyword: "maximum", params: { comparison: "<=", limit: 300 }, message: "must be <= 300" }];
                                                                    return false;
                                                                  } else {
                                                                    if (data38 < 1 || isNaN(data38)) {
                                                                      validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/timeoutSeconds", schemaPath: "#/$defs/connection/properties/timeoutSeconds/minimum", keyword: "minimum", params: { comparison: ">=", limit: 1 }, message: "must be >= 1" }];
                                                                      return false;
                                                                    }
                                                                  }
                                                                }
                                                              }
                                                              var valid10 = _errs77 === errors;
                                                            } else {
                                                              var valid10 = true;
                                                            }
                                                            if (valid10) {
                                                              if (data32.toolAllowlist !== void 0) {
                                                                let data39 = data32.toolAllowlist;
                                                                const _errs79 = errors;
                                                                if (errors === _errs79) {
                                                                  if (Array.isArray(data39)) {
                                                                    var valid11 = true;
                                                                    const len3 = data39.length;
                                                                    for (let i3 = 0; i3 < len3; i3++) {
                                                                      const _errs81 = errors;
                                                                      if (typeof data39[i3] !== "string") {
                                                                        validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/toolAllowlist/" + i3, schemaPath: "#/$defs/connection/properties/toolAllowlist/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                                        return false;
                                                                      }
                                                                      var valid11 = _errs81 === errors;
                                                                      if (!valid11) {
                                                                        break;
                                                                      }
                                                                    }
                                                                  } else {
                                                                    validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/toolAllowlist", schemaPath: "#/$defs/connection/properties/toolAllowlist/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                                                    return false;
                                                                  }
                                                                }
                                                                var valid10 = _errs79 === errors;
                                                              } else {
                                                                var valid10 = true;
                                                              }
                                                              if (valid10) {
                                                                if (data32.desiredEnabled !== void 0) {
                                                                  const _errs83 = errors;
                                                                  if (typeof data32.desiredEnabled !== "boolean") {
                                                                    validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/desiredEnabled", schemaPath: "#/$defs/connection/properties/desiredEnabled/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                                                    return false;
                                                                  }
                                                                  var valid10 = _errs83 === errors;
                                                                } else {
                                                                  var valid10 = true;
                                                                }
                                                                if (valid10) {
                                                                  if (data32.setupRequired !== void 0) {
                                                                    const _errs85 = errors;
                                                                    if (true !== data32.setupRequired) {
                                                                      validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/setupRequired", schemaPath: "#/$defs/connection/properties/setupRequired/const", keyword: "const", params: { allowedValue: true }, message: "must be equal to constant" }];
                                                                      return false;
                                                                    }
                                                                    var valid10 = _errs85 === errors;
                                                                  } else {
                                                                    var valid10 = true;
                                                                  }
                                                                  if (valid10) {
                                                                    if (data32.setupFields !== void 0) {
                                                                      let data43 = data32.setupFields;
                                                                      const _errs86 = errors;
                                                                      if (errors === _errs86) {
                                                                        if (Array.isArray(data43)) {
                                                                          if (data43.length > 7) {
                                                                            validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/setupFields", schemaPath: "#/$defs/connection/properties/setupFields/maxItems", keyword: "maxItems", params: { limit: 7 }, message: "must NOT have more than 7 items" }];
                                                                            return false;
                                                                          } else {
                                                                            if (data43.length < 1) {
                                                                              validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/setupFields", schemaPath: "#/$defs/connection/properties/setupFields/minItems", keyword: "minItems", params: { limit: 1 }, message: "must NOT have fewer than 1 items" }];
                                                                              return false;
                                                                            } else {
                                                                              var valid12 = true;
                                                                              const len4 = data43.length;
                                                                              for (let i4 = 0; i4 < len4; i4++) {
                                                                                let data44 = data43[i4];
                                                                                const _errs88 = errors;
                                                                                if (!(data44 === "roots" || data44 === "url" || data44 === "command" || data44 === "args" || data44 === "authentication" || data44 === "environment" || data44 === "secret_environment")) {
                                                                                  validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/setupFields/" + i4, schemaPath: "#/$defs/connection/properties/setupFields/items/enum", keyword: "enum", params: { allowedValues: schema36.properties.setupFields.items.enum }, message: "must be equal to one of the allowed values" }];
                                                                                  return false;
                                                                                }
                                                                                var valid12 = _errs88 === errors;
                                                                                if (!valid12) {
                                                                                  break;
                                                                                }
                                                                              }
                                                                              if (valid12) {
                                                                                let i5 = data43.length;
                                                                                let j0;
                                                                                if (i5 > 1) {
                                                                                  outer0: for (; i5--; ) {
                                                                                    for (j0 = i5; j0--; ) {
                                                                                      if (func0(data43[i5], data43[j0])) {
                                                                                        validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/setupFields", schemaPath: "#/$defs/connection/properties/setupFields/uniqueItems", keyword: "uniqueItems", params: { i: i5, j: j0 }, message: "must NOT have duplicate items (items ## " + j0 + " and " + i5 + " are identical)" }];
                                                                                        return false;
                                                                                        break outer0;
                                                                                      }
                                                                                    }
                                                                                  }
                                                                                }
                                                                              }
                                                                            }
                                                                          }
                                                                        } else {
                                                                          validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/setupFields", schemaPath: "#/$defs/connection/properties/setupFields/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                                                          return false;
                                                                        }
                                                                      }
                                                                      var valid10 = _errs86 === errors;
                                                                    } else {
                                                                      var valid10 = true;
                                                                    }
                                                                    if (valid10) {
                                                                      if (data32.rootCount !== void 0) {
                                                                        let data45 = data32.rootCount;
                                                                        const _errs89 = errors;
                                                                        if (!(typeof data45 == "number" && (!(data45 % 1) && !isNaN(data45)) && isFinite(data45))) {
                                                                          validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/rootCount", schemaPath: "#/$defs/connection/properties/rootCount/type", keyword: "type", params: { type: "integer" }, message: "must be integer" }];
                                                                          return false;
                                                                        }
                                                                        if (errors === _errs89) {
                                                                          if (typeof data45 == "number" && isFinite(data45)) {
                                                                            if (data45 > 32 || isNaN(data45)) {
                                                                              validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/rootCount", schemaPath: "#/$defs/connection/properties/rootCount/maximum", keyword: "maximum", params: { comparison: "<=", limit: 32 }, message: "must be <= 32" }];
                                                                              return false;
                                                                            } else {
                                                                              if (data45 < 0 || isNaN(data45)) {
                                                                                validate20.errors = [{ instancePath: instancePath + "/connections/" + i2 + "/rootCount", schemaPath: "#/$defs/connection/properties/rootCount/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" }];
                                                                                return false;
                                                                              }
                                                                            }
                                                                          }
                                                                        }
                                                                        var valid10 = _errs89 === errors;
                                                                      } else {
                                                                        var valid10 = true;
                                                                      }
                                                                    }
                                                                  }
                                                                }
                                                              }
                                                            }
                                                          }
                                                        }
                                                      }
                                                    }
                                                  }
                                                }
                                              }
                                            } else {
                                              validate20.errors = [{ instancePath: instancePath + "/connections/" + i2, schemaPath: "#/$defs/connection/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                                              return false;
                                            }
                                          }
                                          var valid8 = _errs64 === errors;
                                          if (!valid8) {
                                            break;
                                          }
                                        }
                                      }
                                    } else {
                                      validate20.errors = [{ instancePath: instancePath + "/connections", schemaPath: "#/properties/connections/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                      return false;
                                    }
                                  }
                                  var valid0 = _errs62 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.bindings !== void 0) {
                                    let data46 = data.bindings;
                                    const _errs91 = errors;
                                    if (errors === _errs91) {
                                      if (Array.isArray(data46)) {
                                        var valid14 = true;
                                        const len5 = data46.length;
                                        for (let i6 = 0; i6 < len5; i6++) {
                                          let data47 = data46[i6];
                                          const _errs93 = errors;
                                          const _errs94 = errors;
                                          if (errors === _errs94) {
                                            if (data47 && typeof data47 == "object" && !Array.isArray(data47)) {
                                              let missing5;
                                              if (data47.id === void 0 && (missing5 = "id") || data47.connectionId === void 0 && (missing5 = "connectionId") || data47.subject === void 0 && (missing5 = "subject") || data47.tools === void 0 && (missing5 = "tools") || data47.allowWrite === void 0 && (missing5 = "allowWrite") || data47.required === void 0 && (missing5 = "required") || data47.namespace === void 0 && (missing5 = "namespace")) {
                                                validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6, schemaPath: "#/$defs/binding/required", keyword: "required", params: { missingProperty: missing5 }, message: "must have required property '" + missing5 + "'" }];
                                                return false;
                                              } else {
                                                const _errs96 = errors;
                                                for (const key5 in data47) {
                                                  if (!(key5 === "id" || key5 === "connectionId" || key5 === "subject" || key5 === "tools" || key5 === "allowWrite" || key5 === "required" || key5 === "namespace")) {
                                                    validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6, schemaPath: "#/$defs/binding/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key5 }, message: "must NOT have additional properties" }];
                                                    return false;
                                                    break;
                                                  }
                                                }
                                                if (_errs96 === errors) {
                                                  if (data47.id !== void 0) {
                                                    const _errs97 = errors;
                                                    if (typeof data47.id !== "string") {
                                                      validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/id", schemaPath: "#/$defs/binding/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                      return false;
                                                    }
                                                    var valid16 = _errs97 === errors;
                                                  } else {
                                                    var valid16 = true;
                                                  }
                                                  if (valid16) {
                                                    if (data47.connectionId !== void 0) {
                                                      const _errs99 = errors;
                                                      if (typeof data47.connectionId !== "string") {
                                                        validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/connectionId", schemaPath: "#/$defs/binding/properties/connectionId/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                        return false;
                                                      }
                                                      var valid16 = _errs99 === errors;
                                                    } else {
                                                      var valid16 = true;
                                                    }
                                                    if (valid16) {
                                                      if (data47.subject !== void 0) {
                                                        let data50 = data47.subject;
                                                        const _errs101 = errors;
                                                        if (errors === _errs101) {
                                                          if (data50 && typeof data50 == "object" && !Array.isArray(data50)) {
                                                            let missing6;
                                                            if (data50.type === void 0 && (missing6 = "type") || data50.id === void 0 && (missing6 = "id")) {
                                                              validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/subject", schemaPath: "#/$defs/binding/properties/subject/required", keyword: "required", params: { missingProperty: missing6 }, message: "must have required property '" + missing6 + "'" }];
                                                              return false;
                                                            } else {
                                                              const _errs103 = errors;
                                                              for (const key6 in data50) {
                                                                if (!(key6 === "type" || key6 === "id")) {
                                                                  validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/subject", schemaPath: "#/$defs/binding/properties/subject/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key6 }, message: "must NOT have additional properties" }];
                                                                  return false;
                                                                  break;
                                                                }
                                                              }
                                                              if (_errs103 === errors) {
                                                                if (data50.type !== void 0) {
                                                                  let data51 = data50.type;
                                                                  const _errs104 = errors;
                                                                  if (!(data51 === "agent" || data51 === "role")) {
                                                                    validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/subject/type", schemaPath: "#/$defs/binding/properties/subject/properties/type/enum", keyword: "enum", params: { allowedValues: schema37.properties.subject.properties.type.enum }, message: "must be equal to one of the allowed values" }];
                                                                    return false;
                                                                  }
                                                                  var valid17 = _errs104 === errors;
                                                                } else {
                                                                  var valid17 = true;
                                                                }
                                                                if (valid17) {
                                                                  if (data50.id !== void 0) {
                                                                    const _errs105 = errors;
                                                                    if (typeof data50.id !== "string") {
                                                                      validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/subject/id", schemaPath: "#/$defs/binding/properties/subject/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                                      return false;
                                                                    }
                                                                    var valid17 = _errs105 === errors;
                                                                  } else {
                                                                    var valid17 = true;
                                                                  }
                                                                }
                                                              }
                                                            }
                                                          } else {
                                                            validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/subject", schemaPath: "#/$defs/binding/properties/subject/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                                                            return false;
                                                          }
                                                        }
                                                        var valid16 = _errs101 === errors;
                                                      } else {
                                                        var valid16 = true;
                                                      }
                                                      if (valid16) {
                                                        if (data47.tools !== void 0) {
                                                          let data53 = data47.tools;
                                                          const _errs107 = errors;
                                                          if (errors === _errs107) {
                                                            if (Array.isArray(data53)) {
                                                              if (data53.length > 1e3) {
                                                                validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/tools", schemaPath: "#/$defs/binding/properties/tools/maxItems", keyword: "maxItems", params: { limit: 1e3 }, message: "must NOT have more than 1000 items" }];
                                                                return false;
                                                              } else {
                                                                if (data53.length < 1) {
                                                                  validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/tools", schemaPath: "#/$defs/binding/properties/tools/minItems", keyword: "minItems", params: { limit: 1 }, message: "must NOT have fewer than 1 items" }];
                                                                  return false;
                                                                } else {
                                                                  var valid18 = true;
                                                                  const len6 = data53.length;
                                                                  for (let i7 = 0; i7 < len6; i7++) {
                                                                    const _errs109 = errors;
                                                                    if (typeof data53[i7] !== "string") {
                                                                      validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/tools/" + i7, schemaPath: "#/$defs/binding/properties/tools/items/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                                      return false;
                                                                    }
                                                                    var valid18 = _errs109 === errors;
                                                                    if (!valid18) {
                                                                      break;
                                                                    }
                                                                  }
                                                                  if (valid18) {
                                                                    let i8 = data53.length;
                                                                    let j1;
                                                                    if (i8 > 1) {
                                                                      const indices0 = {};
                                                                      for (; i8--; ) {
                                                                        let item0 = data53[i8];
                                                                        if (typeof item0 !== "string") {
                                                                          continue;
                                                                        }
                                                                        if (typeof indices0[item0] == "number") {
                                                                          j1 = indices0[item0];
                                                                          validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/tools", schemaPath: "#/$defs/binding/properties/tools/uniqueItems", keyword: "uniqueItems", params: { i: i8, j: j1 }, message: "must NOT have duplicate items (items ## " + j1 + " and " + i8 + " are identical)" }];
                                                                          return false;
                                                                          break;
                                                                        }
                                                                        indices0[item0] = i8;
                                                                      }
                                                                    }
                                                                  }
                                                                }
                                                              }
                                                            } else {
                                                              validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/tools", schemaPath: "#/$defs/binding/properties/tools/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                                              return false;
                                                            }
                                                          }
                                                          var valid16 = _errs107 === errors;
                                                        } else {
                                                          var valid16 = true;
                                                        }
                                                        if (valid16) {
                                                          if (data47.allowWrite !== void 0) {
                                                            const _errs111 = errors;
                                                            if (typeof data47.allowWrite !== "boolean") {
                                                              validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/allowWrite", schemaPath: "#/$defs/binding/properties/allowWrite/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                                              return false;
                                                            }
                                                            var valid16 = _errs111 === errors;
                                                          } else {
                                                            var valid16 = true;
                                                          }
                                                          if (valid16) {
                                                            if (data47.required !== void 0) {
                                                              const _errs113 = errors;
                                                              if (typeof data47.required !== "boolean") {
                                                                validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/required", schemaPath: "#/$defs/binding/properties/required/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" }];
                                                                return false;
                                                              }
                                                              var valid16 = _errs113 === errors;
                                                            } else {
                                                              var valid16 = true;
                                                            }
                                                            if (valid16) {
                                                              if (data47.namespace !== void 0) {
                                                                let data57 = data47.namespace;
                                                                const _errs115 = errors;
                                                                if (errors === _errs115) {
                                                                  if (typeof data57 === "string") {
                                                                    if (func7(data57) > 120) {
                                                                      validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/namespace", schemaPath: "#/$defs/binding/properties/namespace/maxLength", keyword: "maxLength", params: { limit: 120 }, message: "must NOT have more than 120 characters" }];
                                                                      return false;
                                                                    } else {
                                                                      if (func7(data57) < 1) {
                                                                        validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/namespace", schemaPath: "#/$defs/binding/properties/namespace/minLength", keyword: "minLength", params: { limit: 1 }, message: "must NOT have fewer than 1 characters" }];
                                                                        return false;
                                                                      }
                                                                    }
                                                                  } else {
                                                                    validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6 + "/namespace", schemaPath: "#/$defs/binding/properties/namespace/type", keyword: "type", params: { type: "string" }, message: "must be string" }];
                                                                    return false;
                                                                  }
                                                                }
                                                                var valid16 = _errs115 === errors;
                                                              } else {
                                                                var valid16 = true;
                                                              }
                                                            }
                                                          }
                                                        }
                                                      }
                                                    }
                                                  }
                                                }
                                              }
                                            } else {
                                              validate20.errors = [{ instancePath: instancePath + "/bindings/" + i6, schemaPath: "#/$defs/binding/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
                                              return false;
                                            }
                                          }
                                          var valid14 = _errs93 === errors;
                                          if (!valid14) {
                                            break;
                                          }
                                        }
                                      } else {
                                        validate20.errors = [{ instancePath: instancePath + "/bindings", schemaPath: "#/properties/bindings/type", keyword: "type", params: { type: "array" }, message: "must be array" }];
                                        return false;
                                      }
                                    }
                                    var valid0 = _errs91 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        } else {
          validate20.errors = [{ instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" }];
          return false;
        }
      }
      validate20.errors = vErrors;
      return errors === 0;
    }
    validate20.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
  }
});
export default require_configuration_export_validator();
