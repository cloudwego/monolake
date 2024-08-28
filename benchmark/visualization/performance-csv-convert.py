import csv

in_filed_name = ['Case', 'Requests/sec', 'Transfer 10K/sec', 'Server Error', 'Timeout']

csv_filename = "proxies-performance.csv"
with open(csv_filename, 'r') as csvfile:
    reader = csv.reader(csvfile, delimiter=',')
    next(reader)
    d_csv = dict()
    for j, row in enumerate(reader):
        d_row = dict()
        for i, column in enumerate(row):
            if i == 0:
                continue
            d_row[i-1] = column
        d_csv[j] = d_row

# print("d_csv:")
# print(d_csv)

fieldnames = ["Case", 
			  "Tiny Requests/sec", "Small Requests/sec", "Medium Requests/sec", "Large Requests/sec",
              "Tiny Transfer/sec", "Small Transfer/sec", "Medium Transfer/sec", "Large Transfer/sec", ]

o_csv = dict()
o_csv["http-result-4c-monolake"] = list()
o_csv["http-result-4c-nginx"] = list()
o_csv["http-result-4c-traefik"] = list()
o_csv["http-result-4c-envoy"] = list()
o_csv["https-result-4c-monolake"] = list()
o_csv["https-result-4c-nginx"] = list()
o_csv["https-result-4c-traefik"] = list()
o_csv["https-result-4c-envoy"] = list()

o_csv["http-result-4c-monolake"].append("http-result-4c-monolake")
o_csv["http-result-4c-monolake"].append(d_csv[0][0])
o_csv["http-result-4c-monolake"].append(d_csv[1][0])
o_csv["http-result-4c-monolake"].append(d_csv[2][0])
o_csv["http-result-4c-monolake"].append(d_csv[3][0])
o_csv["http-result-4c-monolake"].append(d_csv[0][1])
o_csv["http-result-4c-monolake"].append(d_csv[1][1])
o_csv["http-result-4c-monolake"].append(d_csv[2][1])
o_csv["http-result-4c-monolake"].append(d_csv[3][1])

o_csv["http-result-4c-nginx"].append("http-result-4c-nginx")
o_csv["http-result-4c-nginx"].append(d_csv[4][0])
o_csv["http-result-4c-nginx"].append(d_csv[5][0])
o_csv["http-result-4c-nginx"].append(d_csv[6][0])
o_csv["http-result-4c-nginx"].append(d_csv[7][0])
o_csv["http-result-4c-nginx"].append(d_csv[4][1])
o_csv["http-result-4c-nginx"].append(d_csv[5][1])
o_csv["http-result-4c-nginx"].append(d_csv[6][1])
o_csv["http-result-4c-nginx"].append(d_csv[7][1])

o_csv["http-result-4c-traefik"].append("http-result-4c-traefik")
o_csv["http-result-4c-traefik"].append(d_csv[8][0])
o_csv["http-result-4c-traefik"].append(d_csv[9][0])
o_csv["http-result-4c-traefik"].append(d_csv[10][0])
o_csv["http-result-4c-traefik"].append(d_csv[11][0])
o_csv["http-result-4c-traefik"].append(d_csv[8][1])
o_csv["http-result-4c-traefik"].append(d_csv[9][1])
o_csv["http-result-4c-traefik"].append(d_csv[10][1])
o_csv["http-result-4c-traefik"].append(d_csv[11][1])

o_csv["http-result-4c-envoy"].append("http-result-4c-envoy")
o_csv["http-result-4c-envoy"].append(d_csv[12][0])
o_csv["http-result-4c-envoy"].append(d_csv[13][0])
o_csv["http-result-4c-envoy"].append(d_csv[14][0])
o_csv["http-result-4c-envoy"].append(d_csv[15][0])
o_csv["http-result-4c-envoy"].append(d_csv[12][1])
o_csv["http-result-4c-envoy"].append(d_csv[13][1])
o_csv["http-result-4c-envoy"].append(d_csv[14][1])
o_csv["http-result-4c-envoy"].append(d_csv[15][1])

o_csv["https-result-4c-monolake"].append("https-result-4c-monolake")
o_csv["https-result-4c-monolake"].append(d_csv[16][0])
o_csv["https-result-4c-monolake"].append(d_csv[17][0])
o_csv["https-result-4c-monolake"].append(d_csv[18][0])
o_csv["https-result-4c-monolake"].append(d_csv[19][0])
o_csv["https-result-4c-monolake"].append(d_csv[16][1])
o_csv["https-result-4c-monolake"].append(d_csv[17][1])
o_csv["https-result-4c-monolake"].append(d_csv[18][1])
o_csv["https-result-4c-monolake"].append(d_csv[19][1])

o_csv["https-result-4c-nginx"].append("https-result-4c-nginx")
o_csv["https-result-4c-nginx"].append(d_csv[20][0])
o_csv["https-result-4c-nginx"].append(d_csv[21][0])
o_csv["https-result-4c-nginx"].append(d_csv[22][0])
o_csv["https-result-4c-nginx"].append(d_csv[23][0])
o_csv["https-result-4c-nginx"].append(d_csv[20][1])
o_csv["https-result-4c-nginx"].append(d_csv[21][1])
o_csv["https-result-4c-nginx"].append(d_csv[22][1])
o_csv["https-result-4c-nginx"].append(d_csv[23][1])

o_csv["https-result-4c-traefik"].append("https-result-4c-traefik")
o_csv["https-result-4c-traefik"].append(d_csv[24][0])
o_csv["https-result-4c-traefik"].append(d_csv[25][0])
o_csv["https-result-4c-traefik"].append(d_csv[26][0])
o_csv["https-result-4c-traefik"].append(d_csv[27][0])
o_csv["https-result-4c-traefik"].append(d_csv[24][1])
o_csv["https-result-4c-traefik"].append(d_csv[25][1])
o_csv["https-result-4c-traefik"].append(d_csv[26][1])
o_csv["https-result-4c-traefik"].append(d_csv[27][1])

o_csv["https-result-4c-envoy"].append("https-result-4c-envoy")
o_csv["https-result-4c-envoy"].append(d_csv[28][0])
o_csv["https-result-4c-envoy"].append(d_csv[29][0])
o_csv["https-result-4c-envoy"].append(d_csv[30][0])
o_csv["https-result-4c-envoy"].append(d_csv[31][0])
o_csv["https-result-4c-envoy"].append(d_csv[28][1])
o_csv["https-result-4c-envoy"].append(d_csv[29][1])
o_csv["https-result-4c-envoy"].append(d_csv[30][1])
o_csv["https-result-4c-envoy"].append(d_csv[31][1])

# print("o_csv:")
# print(o_csv)

output_filename1 = "proxies-performance-rotated.csv"

with open(output_filename1, 'w') as csvfile_output:
    writer = csv.writer(csvfile_output, delimiter=',')
    writer.writerow(fieldnames)
    writer.writerow(o_csv["http-result-4c-monolake"])
    writer.writerow(o_csv["http-result-4c-nginx"])
    writer.writerow(o_csv["http-result-4c-traefik"])
    writer.writerow(o_csv["http-result-4c-envoy"])
    writer.writerow(o_csv["https-result-4c-monolake"])
    writer.writerow(o_csv["https-result-4c-nginx"])
    writer.writerow(o_csv["https-result-4c-traefik"])
    writer.writerow(o_csv["https-result-4c-envoy"])
