include "base.thrift"

namespace rs lust.example.item

struct Item {
    1: required i64 id
    2: required string title
    3: required string content
    6: optional list<ObjectChildrenItem> nodes,
}

struct Empty {
}

struct ObjectChildrenItem {
    1: string id
    2: string name
    5: list<ObjectChildrenItem> children
}

struct Foo {
    2: string name
    3: list<Bar> empty
    4: list<Bar> bars
}

struct Bar {
    1: i64 id
    3: set<bool> empty
    4: set<i32> ids
}

struct GetItemRequest {
    1: required i64 id
    2: required string title
    3: optional string content
    4: string name
    6: optional ObjectChildrenItem nodes,
    7: optional Foo foo,
    8: binary base64
    9: double prices
    10: byte  tbytes
    11: bool  tbool
    12: Empty emptystruct
    
    64: optional map<string, string> extra,
    65: binary base64_2
}

struct GetItemWithHttpRequest {
    1: required i64 id
    2: required string title
    3: optional string content
    4: string name
    6: optional ObjectChildrenItem nodes,
    7: optional Foo foo,
    8: binary base64
    9: double prices
    10: byte  tbytes
    11: bool  tbool
    12: Empty emptystruct
    
    20: string Query            (api.query = "query")
    21: list<i32> QueryList     (api.query = "query_list")
    22: string Header           (api.header = "header")
    23: string Cookie           (api.cookie = "cookie")
    24: string RawUri           (agw.source = "raw_uri")
    25: string Path             (api.path = "path")
    26: string BodyDynamic      (agw.source = "body_dynamic")
    28: NotBodyStruct NotBody     (agw.source = "not_body_struct")
    64: optional map<string, string> extra,

    255: base.Base Base
}

struct NotBodyStruct {
    20: string Query            (api.query = "query")
    21: list<i32> QueryList     (api.query = "query_list")
    22: string Header           (api.header = "header")
    23: string Cookie           (api.cookie = "cookie")
    24: string RawUri           (agw.source = "raw_uri")
    25: string Path             (api.path = "path")

    255: base.Base Base
}


struct GetItemResponse {
    1: string Message      (api.body="msg")
    2: string PluginInfo   (api.body="plugin")
    3: Item item,
    4: string ignored_data (agw.target="ignore")

    255: required base.BaseResp  BaseResp
}

struct InnerTestBase {
    255: base.Base Base
}

struct TestBase {
    255: base.Base Base
}

struct TestBaseResp {
    255: base.Base Base
}

struct Root {
    1: string String (agw.source = "body", agw.key = "string_key")
    2: i32 I32 (agw.source = "body")
    3: bool Bool (agw.source = "body")
    4: double Double (agw.source = "body")
}

struct TestPost {
    1: string String (agw.source = "post", agw.key = "string_key")
    2: i32 I32 (agw.source = "post")
    3: bool Bool (agw.source = "post")
    4: double Double (agw.source = "post")
    30: Root Root (agw.source = "root")
}

struct TestJsonRoot {
    1: string String (agw.source = "body", agw.key = "string_key")
    2: i32 I32 (agw.source = "body")
    3: bool Bool (agw.source = "body")
    4: double Double (agw.source = "body")
    30: Root Root (agw.source = "root")
}

struct Headers {
    16: list<string> EmptyList (api.header = "header_empty")
    17: list<string> ValueList (api.header = "header_list")
    18: map<string, list<string>> AllHeaders (agw.source = "headers")
    19: string Value (api.header = "header")
}

struct WithoutSourceInner {
    20: string Query            (agw.key = "query")
    21: list<i32> QueryList     (agw.key = "query_list")
    22: string Header           (agw.key = "header")
    23: string Cookie           (agw.key = "cookie")
    25: string Path             (agw.key = "path")
    26: string jsonkey
    20: string dynamic          (agw.source = "body_dynamic")
}

struct JsonInner {
    20:  string json1            (agw.key = "inner1")
    21:  WithoutSourceInner root (agw.source = "root")
    20:  string dynamic          (agw.source = "body_dynamic")
}

struct WithoutSource {
    20: string Query            (agw.key = "query")
    21: list<i32> QueryList     (agw.key = "query_list")
    22: string Header           (agw.key = "header")
    23: string Cookie           (agw.key = "cookie")
    25: string Path             (agw.key = "path")
    26: WithoutSourceInner not_body_struct (agw.source = "not_body_struct")
    27: JsonInner Json          (agw.key = "json")
}

service ItemService {
    TestBaseResp GetBase (1: TestBase req)
    Empty TestPost (1: TestPost req)
    Empty TestJsonRoot (1: TestJsonRoot req)
    Empty TestHeaders (1: Headers req)
    Empty TestWithoutSource (1: WithoutSource req)
    GetItemResponse GetItem (1: GetItemRequest req)
    GetItemResponse GetItemWithHttp (1: GetItemWithHttpRequest req)
}