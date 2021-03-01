import socket
from time import sleep

UDP_IP = "127.0.0.1"
UDP_PORT = 3400

sock = socket.socket(socket.AF_INET,  # Internet
                     socket.SOCK_DGRAM)  # UDP
sock.bind((UDP_IP, UDP_PORT))


def bytes_dump(idata):
    res = []
    for byte in idata:
        res.append("{:02X}".format(int(byte)))
    return res


while True:
    data, addr = sock.recvfrom(16384)  # buffer size is 1024 bytes
    print(f"received message: {data}\n from {addr} {len(data)} bytes long\nhex {bytes_dump(data)}")
    print(f"echoing... to {addr}")
    sleep(1)
    try:
        sock.sendto(data, addr)
    except Exception as e:
        print(f"echo failed with {e}")
